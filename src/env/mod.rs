use std::{
    collections::HashMap,
    env,
    fs::{self, File},
    io::Write,
    path::PathBuf,
    process::exit,
};

use anyhow::Result;
use curl::easy::Easy;
use sqlx::{Pool, Sqlite};
use virt::{connect::Connect, network::Network};

use crate::{
    Flags, database,
    paths::StoragePaths,
    server::log::{self, Severity, SharedLogger},
};

const USER_ENVVAR: &str = "USER";
const ARCH_DOWNLOAD_URL: &str =
    "https://fastly.mirror.pkgbuild.com/images/latest/Arch-Linux-x86_64-cloudimg.qcow2";
const ANDROMEDA_USERS_NETWORK: &str = include_str!("../../libvirt/andromeda-users.xml");

/// Ensure the environment is sound
pub fn preflight_check(flags: &Flags, paths: &StoragePaths, logger: &SharedLogger) {
    let env: HashMap<String, String> = env::vars().collect();

    ensure_root(&env, logger);

    log::log_message(logger, Severity::INFO, "Verifying libvirt directories");
    if let Err(e) = setup_libvirt_directories(paths) {
        log::log_message(
            logger,
            Severity::FATAL,
            format!("Error when creating libvirt directories: {e}"),
        );
        exit(1)
    };

    log::log_message(logger, Severity::INFO, "Verifying Arch cloud image");
    if let Err(e) = pull_cloud_image(flags, paths, logger) {
        log::log_message(
            logger,
            Severity::FATAL,
            format!("Failed to pull Arch cloud image: {e}"),
        );
        exit(1)
    };

    log::log_message(
        logger,
        Severity::INFO,
        "Connecting to qemu:///system to verify network",
    );
    let qemu = Connect::open(Some("qemu:///system")).unwrap_or_else(|error| {
        log::log_message(
            logger,
            Severity::FATAL,
            format!("Failed to connect to qemu: {error}"),
        );
        exit(1)
    });

    log::log_message(
        logger,
        Severity::INFO,
        "Verifying virtual andromeda-users network",
    );
    if let Err(e) = check_libvirt_network(&qemu, logger) {
        log::log_message(
            logger,
            Severity::FATAL,
            format!("Error when checking virtual network: {e}"),
        );
        exit(1)
    }

    log::log_message(logger, Severity::INFO, "Preflight complete");
}

fn ensure_root(env: &HashMap<String, String>, logger: &SharedLogger) {
    let Some(user) = env.get(USER_ENVVAR) else {
        log::log_message(logger, Severity::FATAL, "$USER is not defined");
        exit(1)
    };

    if user != "root" {
        log::log_message(logger, Severity::FATAL, "This program must be run as root.");
        exit(1)
    }
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
            Severity::INFO,
            "Skipping Arch cloud image download. To update, rerun with --update-image",
        );
        return Ok(());
    }

    if !target_path.exists() {
        log::log_message(
            logger,
            Severity::INFO,
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
                Severity::ERROR,
                format!("Failed to write cloud image: {error}"),
            );
            return Ok(0);
        }
        Ok(data.len())
    })?;

    transfer.perform()?;
    log::log_message(logger, Severity::INFO, "Arch cloud image download complete");
    Ok(())
}

fn check_libvirt_network(qemu: &Connect, logger: &SharedLogger) -> Result<()> {
    let network = match Network::lookup_by_name(qemu, "andromeda-users") {
        Ok(n) => n,
        Err(_) => {
            log::log_message(
                logger,
                Severity::INFO,
                "Virtual network andromeda-users does not exist, creating",
            );
            // Create network
            Network::define_xml(qemu, ANDROMEDA_USERS_NETWORK)?
        }
    };

    if !network.is_active()? {
        log::log_message(
            logger,
            Severity::INFO,
            "Virtual network andromeda-users is not active, starting",
        );
        network.create()?;
    }

    if !network.get_autostart()? {
        log::log_message(
            logger,
            Severity::INFO,
            "Marking virtual network andromeda-users as auto-start",
        );
        network.set_autostart(true)?;
    }

    Ok(())
}

pub async fn ensure_database(logger: &SharedLogger) -> Result<Pool<Sqlite>> {
    log::log_message(
        logger,
        Severity::INFO,
        "Creating /var/lib/andromeda/ if doesn't already exist",
    );
    let database_folder = PathBuf::from("/var/lib/andromeda/");
    fs::create_dir_all(database_folder)?;
    log::log_message(
        logger,
        Severity::INFO,
        "Connecting to /var/lib/andromeda/andromeda.db",
    );
    let database = database::connect("/var/lib/andromeda/andromeda.db").await?;
    log::log_message(
        logger,
        Severity::INFO,
        "Database connected and migrations applied",
    );
    Ok(database)
}
