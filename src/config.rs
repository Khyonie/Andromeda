use anyhow::{Result, bail};

use crate::paths::StoragePaths;

#[derive(Clone, Default)]
pub struct Flags {
    pub update_image: bool,
    pub dry_run: bool,
}

/// Application settings; VM provisioning input lives in `model::VmConfig`.
pub struct Config {
    pub flags: Flags,
    pub paths: StoragePaths,
    pub bind_address: String,
    pub libvirt_uri: String,
}

impl Config {
    pub fn from_args() -> Result<Self> {
        let mut flags = Flags::default();
        for argument in std::env::args().skip(1) {
            match argument.as_str() {
                "--dry" => flags.dry_run = true,
                "--update-image" => flags.update_image = true,
                _ => bail!("Unknown flag {argument}, valid flags are [ --dry, --update-image ]"),
            }
        }
        Ok(Self {
            flags,
            paths: StoragePaths::default(),
            bind_address: "0.0.0.0:9966".into(),
            libvirt_uri: "qemu:///system".into(),
        })
    }
}
