use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf};

use anyhow::Result;
use uuid::Uuid;

use crate::{
    macros::interpolate_str,
    model::VmConfig,
    server::log::{OperationLog, Severity},
};

pub mod disk;

const USER_DATA_SEED: &str = include_str!("../../seeds/user-data");
const NETWORK_CONFIG_SEED: &str = include_str!("../../seeds/network-config");
const META_DATA_SEED: &str = include_str!("../../seeds/meta-data");
const ADMIN_KEY: &str = include_str!("../../keys/khyonie_id_ed25519.pub");

pub fn user_seed(config: &VmConfig) -> String {
    interpolate_str!(
        USER_DATA_SEED,
        vm = config.instance.hostname,
        user_name = config.user.name,
        user_key = config.user.key,
        admin_key = ADMIN_KEY
    )
}

pub fn network_config_seed(config: &VmConfig) -> String {
    interpolate_str!(
        NETWORK_CONFIG_SEED,
        dhcp = config.networking.dhcp,
        mac_triple = config.networking.mac,
        ip = config.networking.ip
    )
}

pub fn metadata_seed(config: &VmConfig) -> String {
    interpolate_str!(META_DATA_SEED, vm = config.instance.hostname)
}

/// Request-local cloud-init inputs, removed on success or error when dropped.
pub struct SeedFiles {
    directory: PathBuf,
    operation: OperationLog,
}

impl SeedFiles {
    pub fn write(
        user_data: &str,
        network_config: &str,
        meta_data: &str,
        operation: &OperationLog,
    ) -> Result<Self> {
        let directory = std::path::absolute(std::env::temp_dir())?
            .join(format!("andromeda-seed-{}", Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&directory)?;
        let files = Self {
            directory,
            operation: operation.clone(),
        };
        fs::write(files.user_data(), user_data)?;
        fs::write(files.network_config(), network_config)?;
        fs::write(files.meta_data(), meta_data)?;
        Ok(files)
    }

    pub fn user_data(&self) -> PathBuf {
        self.directory.join("user-data")
    }

    pub fn network_config(&self) -> PathBuf {
        self.directory.join("network-config")
    }

    pub fn meta_data(&self) -> PathBuf {
        self.directory.join("meta-data")
    }
}

impl Drop for SeedFiles {
    fn drop(&mut self) {
        self.operation.info(format!(
            "Removing seed workspace {}",
            self.directory.display()
        ));
        if let Err(error) = fs::remove_dir_all(&self.directory) {
            self.operation.message(
                Severity::WARNING,
                format!(
                    "Failed to remove seed workspace {}: {error}",
                    self.directory.display()
                ),
            );
        } else {
            self.operation.info("Seed workspace removed");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn seed_workspaces_are_private_isolated_and_cleaned_up() {
        let operation = OperationLog::new(crate::server::log::Logger::shared(), "test", None);
        let first =
            SeedFiles::write("first user", "first network", "first metadata", &operation).unwrap();
        let second = SeedFiles::write(
            "second user",
            "second network",
            "second metadata",
            &operation,
        )
        .unwrap();
        assert_ne!(first.directory, second.directory);
        assert!(first.directory.is_absolute());
        assert_eq!(
            fs::metadata(&first.directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(fs::read_to_string(first.user_data()).unwrap(), "first user");
        assert_eq!(
            fs::read_to_string(first.network_config()).unwrap(),
            "first network"
        );
        assert_eq!(
            fs::read_to_string(first.meta_data()).unwrap(),
            "first metadata"
        );
        let first_directory = first.directory.clone();
        let second_directory = second.directory.clone();
        drop(first);
        assert!(!first_directory.exists());
        assert_eq!(
            fs::read_to_string(second.user_data()).unwrap(),
            "second user"
        );
        drop(second);
        assert!(!second_directory.exists());
    }
}
