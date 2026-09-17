use std::path::{Component, Path, PathBuf};

use anyhow::{Result, ensure};

const LIBVIRT_IMAGES: &str = "/var/lib/libvirt/images";
const ARCH_CLOUD_IMAGE: &str = "Arch-Linux-x86_64-cloudimg.qcow2";

/// The storage layout shared by preflight, image creation, and domain XML.
#[derive(Clone)]
pub struct StoragePaths {
    root: PathBuf,
}

impl Default for StoragePaths {
    fn default() -> Self {
        Self {
            root: PathBuf::from(LIBVIRT_IMAGES),
        }
    }
}

impl StoragePaths {
    pub fn templates(&self) -> PathBuf {
        self.root.join("templates")
    }

    pub fn seeds(&self) -> PathBuf {
        self.root.join("seed")
    }

    pub fn instances(&self) -> PathBuf {
        self.root.join("instances")
    }

    pub fn cloud_image(&self) -> PathBuf {
        self.templates().join(ARCH_CLOUD_IMAGE)
    }

    pub fn instance(&self, hostname: &str) -> Result<InstancePaths> {
        let mut components = Path::new(hostname).components();
        ensure!(
            matches!(components.next(), Some(Component::Normal(_)))
                && components.next().is_none()
                && !hostname.contains(['/', '\\', '\0']),
            "hostname must be a single, non-empty filename component"
        );

        Ok(InstancePaths {
            cloud_image: self.cloud_image(),
            disk: self.instances().join(format!("{hostname}.qcow2")),
            seed_iso: self.seeds().join(format!("{hostname}.iso")),
        })
    }
}

/// Computed once per request so commands and XML refer to the same files.
pub struct InstancePaths {
    pub cloud_image: PathBuf,
    pub disk: PathBuf,
    pub seed_iso: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_hostnames_that_are_not_single_file_names() {
        let paths = StoragePaths::default();
        for hostname in [
            "",
            ".",
            "..",
            "../escape",
            "/absolute",
            "a/b",
            "a/",
            "a\\b",
            "a\0b",
        ] {
            assert!(paths.instance(hostname).is_err(), "{hostname:?}");
        }
        for hostname in ["vm-alice", "vm.example", "vm_01"] {
            assert!(paths.instance(hostname).is_ok(), "{hostname:?}");
        }
    }

    #[test]
    fn domain_xml_uses_the_supplied_storage_layout() {
        let storage = StoragePaths {
            root: PathBuf::from("/custom storage/images & templates"),
        };
        let paths = storage.instance("vm-alice").unwrap();
        let config = crate::model::VmConfig {
            user: crate::model::UserConfig {
                name: "alice".into(),
                key: String::new(),
            },
            networking: crate::model::NetworkConfig {
                ip: String::new(),
                dhcp: true,
                mac: "52:54:00:10:00:01".into(),
                remote_port: 22,
                service_port: 25565
            },
            instance: crate::model::InstanceConfig {
                hostname: "vm-alice".into(),
                disk_size: 1024,
                memory: 512,
                vcpus: 1,
            },
        };
        let (xml, _) = crate::libvirt::generate_domain(&config, &paths).unwrap();
        let domain: crate::libvirt::libvirt_domain::Domain = quick_xml::de::from_str(&xml).unwrap();
        assert_eq!(Path::new(&domain.devices.disks[0].source.file), paths.disk);
        assert_eq!(
            Path::new(&domain.devices.disks[1].source.file),
            paths.seed_iso
        );
        assert_eq!(paths.disk.parent(), Some(storage.instances().as_path()));
        assert_eq!(paths.seed_iso.parent(), Some(storage.seeds().as_path()));
        assert_eq!(
            paths.cloud_image.parent(),
            Some(storage.templates().as_path())
        );
    }
}
