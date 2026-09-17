use std::process::ExitCode;

use anyhow::Result;

use crate::{
    config::Config,
    instances::InstanceService,
    logging::{Logger, Severity, SharedLogger},
};

mod cloud_init;
mod config;
mod database;
mod instances;
mod libvirt;
mod logging;
mod macros;
mod model;
mod paths;
mod server;
mod startup;
mod storage;

#[tokio::main]
async fn main() -> ExitCode {
    let logger = Logger::shared();
    let config = match Config::from_args() {
        Ok(config) => config,
        Err(error) => {
            logging::log_message(&logger, Severity::Fatal, error.to_string());
            return ExitCode::FAILURE;
        }
    };
    let result = run(&config, &logger).await;
    if let Err(error) = &result {
        logging::log_message(&logger, Severity::Fatal, format!("{error:#}"));
    }

    // Save after the server drains requests, including their final progress messages.
    let log_folder = config.paths.logs();
    if log_folder.is_dir()
        && let Err(error) = logging::finalize_log(&logger, &log_folder)
    {
        logging::log_message(
            &logger,
            Severity::Error,
            format!("Failed to save log: {error}"),
        );
    }

    if result.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

async fn run(config: &Config, logger: &SharedLogger) -> Result<()> {
    let resources = startup::initialize(config, logger).await?;
    let instances = InstanceService::new(
        resources.database,
        resources.qemu,
        config.flags.clone(),
        config.paths.clone(),
        logger.clone(),
    );
    server::start_server(&config.bind_address, instances).await
}
