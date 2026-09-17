use anyhow::{Context, Result};
use uuid::Uuid;
use virt::{connect::Connect, domain::Domain as LiveDomain, error::Error, network::Network};

use crate::{
    libvirt::xml::{Channel, Console, Disk, Domain, Interface, Mac},
    logging::{self as log, Severity, SharedLogger},
    model::VmConfig,
    paths::InstancePaths,
};

const ANDROMEDA_USERS_NETWORK: &str = include_str!("../../assets/libvirt/andromeda-users.xml");

pub mod xml;

pub fn generate_domain(config: &VmConfig, paths: &InstancePaths) -> Result<(String, Uuid)> {
    let hostname = config.instance.hostname.clone();
    let uuid = Uuid::new_v4();
    let mut domain = Domain::new(
        hostname.clone(),
        config.instance.memory,
        config.instance.vcpus,
    );
    domain.uuid = Some(uuid.into());
    domain.devices.disks = vec![
        Disk::file(
            paths
                .disk
                .to_str()
                .context("disk path is not valid UTF-8 for libvirt XML")?,
            "sdb",
            "qcow2",
        ),
        Disk::cdrom(
            paths
                .seed_iso
                .to_str()
                .context("seed ISO path is not valid UTF-8 for libvirt XML")?,
            "sda",
        ),
    ];
    let mut interface = Interface::network("andromeda-users");
    interface.mac = Some(Mac {
        address: config.networking.mac.clone(),
    });
    domain.devices.interfaces.push(interface);
    domain.devices.consoles.push(Console::serial());
    domain.devices.channels.push(Channel::guest_agent());

    Ok((domain.to_xml()?, uuid))
}

/// Look up a live domain by the UUID persisted with the instance record.
pub fn lookup_domain(qemu: &Connect, id: &str) -> Result<LiveDomain, Error> {
    LiveDomain::lookup_by_uuid_string(qemu, id)
}

pub fn connect(uri: &str) -> Result<Connect, Error> {
    Connect::open(Some(uri))
}

pub fn define_domain(qemu: &Connect, xml: &str) -> Result<(), Error> {
    LiveDomain::define_xml(qemu, xml).map(|_| ())
}

pub fn is_active(domain: &LiveDomain) -> Result<bool, Error> {
    domain.is_active()
}

pub fn undefine_domain(domain: &LiveDomain) -> Result<(), Error> {
    domain.undefine().map(|_| ())
}

pub fn start_domain(domain: &LiveDomain) -> Result<(), Error> {
    domain.create().map(|_| ())
}

pub fn stop_domain(domain: &LiveDomain) -> Result<(), Error> {
    domain.destroy().map(|_| ())
}

pub fn ensure_network(qemu: &Connect, logger: &SharedLogger) -> Result<()> {
    let network = match Network::lookup_by_name(qemu, "andromeda-users") {
        Ok(n) => n,
        Err(_) => {
            log::log_message(
                logger,
                Severity::Info,
                "Virtual network andromeda-users does not exist, creating",
            );
            // Create network
            Network::define_xml(qemu, ANDROMEDA_USERS_NETWORK)?
        }
    };

    if !network.is_active()? {
        log::log_message(
            logger,
            Severity::Info,
            "Virtual network andromeda-users is not active, starting",
        );
        network.create()?;
    }

    if !network.get_autostart()? {
        log::log_message(
            logger,
            Severity::Info,
            "Marking virtual network andromeda-users as auto-start",
        );
        network.set_autostart(true)?;
    }

    Ok(())
}
