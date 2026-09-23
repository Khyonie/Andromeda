use anyhow::{Context, Result};
use axum::{
    Router,
    routing::{delete, get, post},
};
use tokio::net::TcpListener;

use crate::{
    instances::InstanceService,
    logging::{self as log, Severity, SharedLogger},
};

mod admin;
mod auth;
mod error;
mod instance;
mod requests;
mod state;

use state::AppState;

fn router(
    instances: InstanceService,
    auth: crate::auth::AuthService,
    settings: crate::settings::SettingsService,
) -> Router<()> {
    Router::<AppState>::new()
        .route("/api/session", get(auth::session))
        .route("/api/auth/discord", get(auth::login))
        .route("/api/auth/discord/callback", get(auth::callback))
        .route("/api/auth/logout", post(auth::logout))
        .route(
            "/api/admin/settings",
            get(admin::get_settings)
                .put(admin::update_settings)
                .layer(axum::extract::DefaultBodyLimit::max(32 * 1024)),
        )
        .route(
            "/api/instance",
            get(instance::get_instance_ids)
                .put(instance::create_instance)
                .delete(instance::delete_instance),
        )
        .route("/api/instance/id", get(instance::get_instance))
        .route("/api/instance/status", post(instance::get_status))
        .route("/api/instance/disk", post(instance::download_disk))
        .route("/api/instance/start", post(instance::start_instance))
        .route("/api/instance/stop", post(instance::stop_instance))
        .route("/api/instance/stop", delete(instance::destroy_instance))
        .layer(axum::middleware::map_response(
            |mut response: axum::response::Response| async move {
                response
                    .headers_mut()
                    .insert("cache-control", "no-store".parse().unwrap());
                response
                    .headers_mut()
                    .insert("referrer-policy", "no-referrer".parse().unwrap());
                response
            },
        ))
        .with_state::<()>(AppState {
            instances,
            auth,
            settings,
        })
}

pub async fn start_server(
    bind_address: &str,
    instances: InstanceService,
    auth: crate::auth::AuthService,
    settings: crate::settings::SettingsService,
) -> Result<()> {
    let logger = instances.logger.clone();
    let router: Router<()> = router(instances, auth, settings);
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
