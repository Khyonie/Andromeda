use axum::{Json, extract::State, http::StatusCode};

use super::{auth::AdminSession, error::ApiError, state::AppState};
use crate::settings::{GuestDefaults, UpdateDefaults};

pub(super) async fn get_settings(
    State(state): State<AppState>,
    AdminSession(_session): AdminSession,
) -> Result<Json<GuestDefaults>, ApiError> {
    state
        .settings
        .snapshot()
        .await
        .map(Json)
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Could not load guest defaults.".into(),
        })
}

pub(super) async fn update_settings(
    State(state): State<AppState>,
    AdminSession(session): AdminSession,
    Json(input): Json<UpdateDefaults>,
) -> Result<Json<GuestDefaults>, ApiError> {
    state
        .settings
        .update(input, &session.account.id)
        .await
        .map(Json)
        .map_err(Into::into)
}
