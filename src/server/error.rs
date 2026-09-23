use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::instances::{ErrorKind, InstanceError};

pub(super) struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl From<InstanceError> for ApiError {
    fn from(error: InstanceError) -> Self {
        let status = match error.kind {
            ErrorKind::Forbidden => StatusCode::FORBIDDEN,
            ErrorKind::InvalidInput => StatusCode::BAD_REQUEST,
            ErrorKind::NotFound => StatusCode::NOT_FOUND,
            ErrorKind::Conflict => StatusCode::CONFLICT,
            ErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self {
            status,
            message: error.message,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, self.message).into_response()
    }
}
