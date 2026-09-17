use axum::{
    Router,
    extract::FromRef,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
};
use sqlx::SqlitePool;
use tokio::net::TcpListener;
use virt::connect::Connect;

use crate::{
    Flags,
    paths::StoragePaths,
    server::log::{Severity, SharedLogger},
};

mod instance;
pub mod log;

pub const BIND_ADDRESS: &str = "0.0.0.0:9966";

#[derive(Clone)]
pub(crate) struct AppState {
    qemu: Connect,
    flags: Flags,
    database: SqlitePool,
    logger: SharedLogger,
    paths: StoragePaths,
}

impl FromRef<AppState> for SqlitePool {
    fn from_ref(state: &AppState) -> Self {
        state.database.clone()
    }
}

impl FromRef<AppState> for SharedLogger {
    fn from_ref(state: &AppState) -> Self {
        state.logger.clone()
    }
}

pub(crate) struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, self.message).into_response()
    }
}

pub async fn start_server(
    flags: Flags,
    database: SqlitePool,
    paths: StoragePaths,
    logger: SharedLogger,
) -> Result<(), String> {
    log::log_message(&logger, Severity::INFO, "Connecting to qemu:///system");
    let qemu = Connect::open(Some("qemu:///system"))
        .map_err(|e| format!("Failed to connect to qemu: {e}"))?;
    let state = AppState {
        qemu,
        flags,
        database,
        paths,
        logger: logger.clone(),
    };

    let router = Router::new()
        .route("/api/instance", get(instance::get_instance))
        .route("/api/instance/ids", get(instance::get_instance_ids))
        .route("/api/instance", put(instance::create_instance))
        .route("/api/instance", delete(instance::delete_instance))
        .route("/api/instance/start", post(instance::start_instance))
        .route("/api/instance/stop", post(instance::stop_instance))
        .with_state(state);

    log::log_message(
        &logger,
        Severity::INFO,
        format!("Binding management server to {BIND_ADDRESS}"),
    );
    let listener = TcpListener::bind(BIND_ADDRESS)
        .await
        .map_err(|error| format!("Failed to bind server to {BIND_ADDRESS}: {error}"))?;

    log::log_message(
        &logger,
        Severity::INFO,
        "Andromeda management server started",
    );
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal(logger.clone()))
        .await
        .map_err(|e| format!("Failed to start server: {e}"))?;

    log::log_message(&logger, Severity::INFO, "Management server stopped");
    Ok(())
}

async fn shutdown_signal(logger: SharedLogger) {
    if let Err(error) = tokio::signal::ctrl_c().await {
        log::log_message(
            &logger,
            Severity::ERROR,
            format!("Failed to listen for shutdown signal: {error}"),
        );
    } else {
        log::log_message(
            &logger,
            Severity::INFO,
            "Shutdown requested; waiting for active requests",
        );
    }
}
