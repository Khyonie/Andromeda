//! Read-only creation checks and cleanup for resources owned by one attempt.
use std::{fs, net::Ipv4Addr, path::PathBuf};

use anyhow::{Context, Result, ensure};
use virt::{domain::Domain, error::ErrorNumber, network::Network};

use crate::{
    logging::{OperationLog, Severity},
    model::VmConfig,
    paths::InstancePaths,
};

pub(super) fn validate_request(config: &mut VmConfig) -> Result<()> {
    let instance = &config.instance;
    let hostname = instance.hostname.as_bytes();
    ensure!(
        !hostname.is_empty()
            && hostname.len() <= 63
            && hostname[0].is_ascii_alphanumeric()
            && hostname[hostname.len() - 1].is_ascii_alphanumeric()
            && hostname
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || *b == b'-'),
        "Hostname must contain 1–63 letters, numbers, or hyphens, beginning and ending with a letter or number."
    );
    ensure!(
        (1..=u32::MAX as u64).contains(&instance.memory),
        "Memory must be between 1 and 4294967295 MiB."
    );
    ensure!(instance.vcpus > 0, "At least one virtual CPU is required.");
    ensure!(
        instance.disk_size > 0
            && (instance.disk_size as u64)
                .checked_mul(1024 * 1024)
                .is_some_and(|bytes| bytes <= i64::MAX as u64),
        "Disk size must be positive and fit within the supported image size."
    );
    let name = config.user.name.as_bytes();
    ensure!(
        !name.is_empty()
            && name.len() <= 32
            && (name[0].is_ascii_lowercase() || name[0] == b'_')
            && name
                .iter()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-".contains(b)),
        "Username must contain 1–32 lowercase letters, numbers, underscores, or hyphens, beginning with a letter or underscore."
    );
    ensure!(
        !matches!(config.user.name.as_str(), "root" | "sysadmin"),
        "The usernames root and sysadmin are reserved."
    );
    config.user.key = config.user.key.trim().to_owned();
    ensure!(
        !config.user.key.is_empty(),
        "An SSH public key is required."
    );

    let network = &mut config.networking;
    ensure!(
        network.remote_port > 0 && network.service_port > 0,
        "Remote and service ports must be between 1 and 65535."
    );
    let (address, prefix) = network
        .ip
        .trim()
        .split_once('/')
        .context("IP address must include an IPv4 prefix, for example 10.0.0.200/24.")?;
    let address: Ipv4Addr = address.parse().context("Invalid IPv4 address.")?;
    let prefix: u8 = prefix.parse().context("Invalid IPv4 prefix.")?;
    ensure!(prefix <= 32, "IPv4 prefix must be between 0 and 32.");
    ensure!(
        !address.is_unspecified()
            && !address.is_loopback()
            && !address.is_multicast()
            && !address.is_broadcast(),
        "Use a unicast IPv4 address for the instance."
    );
    if prefix < 31 {
        let host_mask = u32::MAX >> prefix;
        let host = u32::from(address) & host_mask;
        ensure!(
            host != 0 && host != host_mask,
            "The instance IP cannot be a network or broadcast address."
        );
    }
    network.ip = format!("{address}/{prefix}");
    let octets = network
        .mac
        .split(':')
        .map(|part| {
            ensure!(
                part.len() == 2,
                "MAC address must contain six hexadecimal pairs separated by colons."
            );
            u8::from_str_radix(part, 16).context("Invalid MAC address.")
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        octets.len() == 6 && octets[0] & 1 == 0 && octets.iter().any(|b| *b != 0),
        "Use a nonzero unicast MAC address with six hexadecimal pairs."
    );
    network.mac = octets
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(":");
    Ok(())
}

pub(super) fn check_domain_name(qemu: &virt::connect::Connect, hostname: &str) -> Result<bool> {
    match Domain::lookup_by_name(qemu, hostname) {
        Ok(_) => Ok(true),
        Err(error) if error.code() == ErrorNumber::NoDomain => Ok(false),
        Err(error) => Err(error).context("Checking for an existing libvirt domain"),
    }
}

pub(super) fn check_hypervisor(qemu: &virt::connect::Connect, config: &VmConfig) -> Result<()> {
    ensure!(qemu.is_alive()?, "The libvirt connection is unavailable.");
    let network = Network::lookup_by_name(qemu, "andromeda-users")
        .context("The andromeda-users libvirt network is missing")?;
    ensure!(
        network.is_active()?,
        "The andromeda-users libvirt network is inactive."
    );
    // Query the actual machine type used by generate_domain without defining anything.
    let xml = qemu
        .get_domain_capabilities(None, Some("x86_64"), Some("q35"), Some("kvm"), 0)
        .context("The host cannot provide the required x86_64/q35 KVM guest")?;
    #[derive(serde::Deserialize)]
    struct Capabilities {
        vcpu: Vcpu,
    }
    #[derive(serde::Deserialize)]
    struct Vcpu {
        #[serde(rename = "@max")]
        max: u32,
    }
    let capabilities: Capabilities = quick_xml::de::from_str(&xml)?;
    ensure!(
        config.instance.vcpus <= capabilities.vcpu.max,
        "This guest supports at most {} virtual CPUs.",
        capabilities.vcpu.max
    );
    Ok(())
}

/// Only files exclusively reserved by this attempt and its returned domain handle
/// can be removed. Existing resources are never adopted for cleanup.
pub(super) struct Resources {
    files: Vec<PathBuf>,
    pub domain: Option<Domain>,
    operation: OperationLog,
    committed: bool,
}

impl Resources {
    pub fn reserve(paths: &InstancePaths, operation: &OperationLog) -> Result<Self> {
        let mut resources = Self {
            files: Vec::new(),
            domain: None,
            operation: operation.clone(),
            committed: false,
        };
        for path in [&paths.disk, &paths.seed_iso] {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .with_context(|| format!("Reserving {}", path.display()))?;
            resources.files.push(path.clone());
        }
        Ok(resources)
    }

    pub fn commit(&mut self) {
        self.committed = true;
    }

    pub fn rollback(&mut self) -> Result<()> {
        if self.committed {
            return Ok(());
        }
        self.operation
            .step("Rolling back incomplete instance creation");
        if let Some(domain) = &self.domain {
            // Never remove storage from a domain unexpectedly started by another actor.
            ensure!(
                !domain.is_active()?,
                "Incomplete domain is active; retaining its storage for manual recovery."
            );
            domain
                .undefine()
                .context("Could not undefine incomplete domain; retaining its storage")?;
            self.domain = None;
        }
        let mut failures = Vec::new();
        self.files.retain(|path| {
            let result = fs::symlink_metadata(path).and_then(|metadata| {
                if !metadata.is_file() {
                    return Err(std::io::Error::other("refusing a non-regular file"));
                }
                fs::remove_file(path)
            });
            match result {
                Ok(()) => {
                    self.operation
                        .info(format!("Removed incomplete file {}", path.display()));
                    false
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
                Err(error) => {
                    failures.push(format!("{}: {error}", path.display()));
                    true
                }
            }
        });
        ensure!(
            failures.is_empty(),
            "Cleanup failed: {}",
            failures.join("; ")
        );
        self.committed = true;
        self.operation.info("Incomplete instance resources removed");
        Ok(())
    }
}

impl Drop for Resources {
    fn drop(&mut self) {
        if let Err(error) = self.rollback() {
            self.operation.message(
                Severity::Error,
                format!("Creation rollback needs manual recovery: {error:#}"),
            );
        }
    }
}
