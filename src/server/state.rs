use axum::extract::FromRef;
use sqlx::SqlitePool;

use crate::{instances::InstanceService, logging::SharedLogger};

#[derive(Clone)]
pub(super) struct AppState {
    pub instances: InstanceService,
    pub auth: crate::auth::AuthService,
    pub settings: crate::settings::SettingsService,
}

impl FromRef<AppState> for SqlitePool {
    fn from_ref(state: &AppState) -> Self {
        state.instances.database.clone()
    }
}

impl FromRef<AppState> for SharedLogger {
    fn from_ref(state: &AppState) -> Self {
        state.instances.logger.clone()
    }
}
