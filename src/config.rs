use std::{env, net::Ipv4Addr};

use anyhow::{Context, Result, bail};

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
    pub auth: crate::auth::AuthConfig,
    pub gateway_ip: Ipv4Addr,
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
            auth: crate::auth::AuthConfig::from_env()?,
            gateway_ip: match env::var("ANDROMEDA_GATEWAY_IP") {
                Ok(value) => value.parse().context(
                    "ANDROMEDA_GATEWAY_IP must be an IPv4 address, for example 10.0.0.1",
                )?,
                Err(env::VarError::NotPresent) => Ipv4Addr::new(10, 0, 0, 1),
                Err(error) => return Err(error).context("Invalid ANDROMEDA_GATEWAY_IP"),
            },
            flags,
            paths: StoragePaths::default(),
            bind_address: "0.0.0.0:9966".into(),
            libvirt_uri: "qemu:///system".into(),
        })
    }
}
