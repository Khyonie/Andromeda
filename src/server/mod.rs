use anyhow::{Context, Result};
use axum::{
    Router,
    routing::{get, post},
};
use tokio::net::TcpListener;

use crate::{
    instances::InstanceService,
    logging::{self as log, Severity, SharedLogger},
};

mod error;
mod instance;
mod requests;
mod state;

use state::AppState;

fn router(instances: InstanceService) -> Router {
    Router::new()
        .route(
            "/api/instance",
            get(instance::get_instance)
                .put(instance::create_instance)
                .delete(instance::delete_instance),
        )
        .route("/api/instance/ids", get(instance::get_instance_ids))
        .route("/api/instance/start", post(instance::start_instance))
        .route("/api/instance/stop", post(instance::stop_instance))
        .with_state(AppState { instances })
}

pub async fn start_server(bind_address: &str, instances: InstanceService) -> Result<()> {
    let logger = instances.logger.clone();
    let router = router(instances);
    log::log_message(
        &logger,
        Severity::Info,
        format!("Binding management server to {bind_address}"),
    );
    let listener = TcpListener::bind(bind_address)
        .await
        .with_context(|| format!("Failed to bind server to {bind_address}"))?;

    log::log_message(
        &logger,
        Severity::Info,
        "Andromeda management server started",
    );
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal(logger.clone()))
        .await
        .context("Failed to run server")?;
    log::log_message(&logger, Severity::Info, "Management server stopped");
    Ok(())
}

async fn shutdown_signal(logger: SharedLogger) {
    if let Err(error) = tokio::signal::ctrl_c().await {
        log::log_message(
            &logger,
            Severity::Error,
            format!("Failed to listen for shutdown signal: {error}"),
        );
    } else {
        log::log_message(
            &logger,
            Severity::Info,
            "Shutdown requested; waiting for active requests",
        );
    }
}
