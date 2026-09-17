use std::process::exit;

use crate::{
    paths::StoragePaths,
    server::log::{self, Logger, Severity, SharedLogger},
};

mod database;
mod env;
mod libvirt;
mod macros;
mod model;
mod paths;
mod seed;
mod server;

#[derive(Clone)]
pub struct Flags {
    update_image: bool,
    dry_run: bool,
}

#[tokio::main]
async fn main() {
    let logger = Logger::shared();
    let flags = parse_args(&logger);

    let paths = StoragePaths::default();
    env::preflight_check(&flags, &paths, &logger);
    let database = match env::ensure_database(&logger).await {
        Ok(database) => database,
        Err(error) => {
            log::log_message(
                &logger,
                Severity::FATAL,
                format!("Failed to open database: {error}"),
            );
            exit(1)
        }
    };

    if let Err(e) = server::start_server(flags, database, paths, logger.clone()).await {
        log::log_message(
            &logger,
            Severity::FATAL,
            format!("Failed to start server: {e}"),
        );
        exit(1)
    };
}

fn parse_args(logger: &SharedLogger) -> Flags {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut flags = Flags {
        update_image: false,
        dry_run: false,
    };

    for a in args {
        match a.as_str() {
            "--dry" => flags.dry_run = true,
            "--update-image" => flags.update_image = true,
            _ => {
                log::log_message(
                    logger,
                    Severity::FATAL,
                    format!("Unknown flag {a}, valid flags are [ --dry, --update-image ]"),
                );
                exit(1)
            }
        }
    }

    flags
}
