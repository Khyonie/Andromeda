use std::{
    env,
    fs::{self, File},
    io::Write,
};

use anyhow::{Context, Result, ensure};
use curl::easy::Easy;
use sqlx::SqlitePool;
use virt::connect::Connect;

use crate::{
    config::{Config, Flags},
    database, libvirt,
    logging::{self as log, Severity, SharedLogger},
    paths::StoragePaths,
};

const ARCH_DOWNLOAD_URL: &str =
    "https://fastly.mirror.pkgbuild.com/images/latest/Arch-Linux-x86_64-cloudimg.qcow2";

pub struct Resources {
    pub database: SqlitePool,
    pub qemu: Connect,
}

/// Prepare the host and open dependencies. The entry point handles fatal errors.
pub async fn initialize(config: &Config, logger: &SharedLogger) -> Result<Resources> {
    let user = env::var("USER").context("$USER is not defined")?;
    ensure!(user == "root", "This program must be run as root.");

    log::log_message(logger, Severity::Info, "Verifying libvirt directories");
    setup_libvirt_directories(&config.paths).context("Error when creating libvirt directories")?;
    log::log_message(logger, Severity::Info, "Verifying Arch cloud image");
    pull_cloud_image(&config.flags, &config.paths, logger)
        .context("Failed to pull Arch cloud image")?;

    log::log_message(
        logger,
        Severity::Info,
        format!("Connecting to {} to verify network", config.libvirt_uri),
    );
    let qemu = libvirt::connect(&config.libvirt_uri).context("Failed to connect to qemu")?;
    log::log_message(
        logger,
        Severity::Info,
        "Verifying virtual andromeda-users network",
    );
    libvirt::ensure_network(&qemu, logger).context("Error when checking virtual network")?;

    log::log_message(
        logger,
        Severity::Info,
        format!(
            "Creating {} if it doesn't already exist",
            config.paths.logs().display()
        ),
    );
    fs::create_dir_all(config.paths.logs()).context("Error when checking log folder")?;
    log::log_message(logger, Severity::Info, "Preflight complete");

    let database = ensure_database(&config.paths, logger)
        .await
        .context("Failed to open database")?;
    Ok(Resources { database, qemu })
}

fn setup_libvirt_directories(paths: &StoragePaths) -> Result<()> {
    fs::create_dir_all(paths.templates())?;
    fs::create_dir_all(paths.seeds())?;
    fs::create_dir_all(paths.instances())?;

    Ok(())
}

fn pull_cloud_image(flags: &Flags, paths: &StoragePaths, logger: &SharedLogger) -> Result<()> {
    let target_path = paths.cloud_image();

    if target_path.exists() && !flags.update_image {
        log::log_message(
            logger,
            Severity::Info,
            "Skipping Arch cloud image download. To update, rerun with --update-image",
        );
        return Ok(());
    }

    if !target_path.exists() {
        log::log_message(
            logger,
            Severity::Info,
            "Arch cloud image missing, downloading image",
        );
    }

    let mut file = File::create(target_path)?;
    let mut downloader = Easy::new();

    downloader.url(ARCH_DOWNLOAD_URL)?;

    let mut transfer = downloader.transfer();
    transfer.write_function(|data| {
        if let Err(error) = file.write_all(data) {
            log::log_message(
                logger,
                Severity::Error,
                format!("Failed to write cloud image: {error}"),
            );
            return Ok(0);
        }
        Ok(data.len())
    })?;

    transfer.perform()?;
    log::log_message(logger, Severity::Info, "Arch cloud image download complete");
    Ok(())
}

async fn ensure_database(paths: &StoragePaths, logger: &SharedLogger) -> Result<SqlitePool> {
    log::log_message(
        logger,
        Severity::Info,
        format!(
            "Creating {} if it doesn't already exist",
            paths.data_directory().display()
        ),
    );
    fs::create_dir_all(paths.data_directory())?;
    log::log_message(
        logger,
        Severity::Info,
        format!("Connecting to {}", paths.database().display()),
    );
    let database = database::connect(paths.database()).await?;
    log::log_message(
        logger,
        Severity::Info,
        "Database connected and migrations applied",
    );
    Ok(database)
}
