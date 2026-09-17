use axum::extract::FromRef;
use sqlx::SqlitePool;

use crate::{instances::InstanceService, logging::SharedLogger};

#[derive(Clone)]
pub(super) struct AppState {
    pub instances: InstanceService,
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
