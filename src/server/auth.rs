use super::{error::ApiError, state::AppState};
use crate::auth::Session;
use axum::{
    Json,
    extract::{FromRequestParts, Query, State},
    http::{HeaderMap, Method, StatusCode, header, request::Parts},
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;

pub struct CurrentSession(pub Session);
pub(super) struct AdminSession(pub Session);

impl FromRequestParts<AppState> for AdminSession {
    type Rejection = ApiError;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let CurrentSession(session) = CurrentSession::from_request_parts(parts, state).await?;
        if !state.auth.config.is_admin(&session.account) {
            return Err(error(
                StatusCode::FORBIDDEN,
                "System administrator access is required.",
            ));
        }
        Ok(Self(session))
    }
}

fn error(status: StatusCode, message: &str) -> ApiError {
    ApiError {
        status,
        message: message.into(),
    }
}

// Only read the named cookie. Reject duplicates rather than choosing an ambiguous identity.
fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let values: Vec<_> = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|h| h.to_str().ok())
        .flat_map(|h| h.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .filter(|(key, _)| *key == name)
        .map(|(_, value)| value.to_owned())
        .collect();
    if values.len() == 1 {
        values.into_iter().next()
    } else {
        None
    }
}

impl FromRequestParts<AppState> for CurrentSession {
    type Rejection = ApiError;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = cookie(&parts.headers, state.auth.config.session_cookie());
        let session = state
            .auth
            .session(token.as_deref())
            .await
            .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Could not read session"))?
            .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "Please sign in"))?;
        if parts.method != Method::GET && parts.method != Method::HEAD {
            let csrf = parts
                .headers
                .get("x-csrf-token")
                .and_then(|h| h.to_str().ok())
                .unwrap_or_default();
            let expected = session.csrf_token.as_bytes();
            let valid = csrf.len() == expected.len()
                && csrf
                    .as_bytes()
                    .iter()
                    .zip(expected)
                    .fold(0u8, |diff, (a, b)| diff | (a ^ b))
                    == 0;
            let origin_ok = parts
                .headers
                .get(header::ORIGIN)
                .is_none_or(|h| h.to_str().ok() == Some(state.auth.config.origin.as_str()));
            if !origin_ok {
                return Err(error(
                    StatusCode::FORBIDDEN,
                    &format!(
                        "This browser address does not match the server's public origin ({}). Open that address, or update ANDROMEDA_PUBLIC_ORIGIN and the Discord redirect URI, then restart the backend and sign in again.",
                        state.auth.config.origin
                    ),
                ));
            }
            if !valid {
                return Err(error(
                    StatusCode::FORBIDDEN,
                    "Your request verification token is out of date. Refresh the page and try again.",
                ));
            }
        }
        Ok(Self(session))
    }
}

pub(super) async fn session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let token = cookie(&headers, state.auth.config.session_cookie());
    let session = state
        .auth
        .session(token.as_deref())
        .await
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Could not read session"))?;
    Ok(Json(match session {
        Some(session) => {
            serde_json::json!({"is_admin": state.auth.config.is_admin(&session.account), "user": session.account, "csrf_token": session.csrf_token, "login_available": state.auth.config.enabled()})
        }
        None => {
            serde_json::json!({"is_admin": false, "user": null, "csrf_token": null, "login_available": state.auth.config.enabled()})
        }
    }))
}

pub(super) async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if !state.auth.config.enabled() {
        return Err(error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Discord sign-in is not configured",
        ));
    }
    let old = cookie(&headers, state.auth.config.login_cookie());
    let (url, cookie) = state
        .auth
        .begin_login(old.as_deref())
        .await
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Could not start sign-in"))?;
    let mut response = Redirect::to(&url).into_response();
    response
        .headers_mut()
        .append(header::SET_COOKIE, cookie.parse().unwrap());
    Ok(response)
}

#[derive(Deserialize)]
pub(super) struct Callback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

pub(super) async fn callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<Callback>,
) -> Response {
    let browser = cookie(&headers, state.auth.config.login_cookie()).unwrap_or_default();
    let valid = state
        .auth
        .consume_login(query.state.as_deref().unwrap_or_default(), &browser)
        .await
        .unwrap_or(false);
    let old_session = cookie(&headers, state.auth.config.session_cookie());
    let result = if valid && query.error.is_none() && state.auth.config.enabled() {
        if let Some(code) = query.code {
            state
                .auth
                .finish_login(code, old_session.as_deref())
                .await
                .ok()
        } else {
            None
        }
    } else {
        None
    };
    let mut response = if let Some(cookie) = result {
        let mut response = Redirect::to(&format!("{}/", state.auth.config.origin)).into_response();
        response
            .headers_mut()
            .append(header::SET_COOKIE, cookie.parse().unwrap());
        response
    } else {
        state.auth.failure();
        Redirect::to(&format!("{}/login?error=discord", state.auth.config.origin)).into_response()
    };
    response.headers_mut().append(
        header::SET_COOKIE,
        state
            .auth
            .config
            .cookie(state.auth.config.login_cookie(), "", 0)
            .parse()
            .unwrap(),
    );
    response
}

pub(super) async fn logout(
    State(state): State<AppState>,
    CurrentSession(session): CurrentSession,
) -> Result<Response, ApiError> {
    state
        .auth
        .logout(&session)
        .await
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Could not sign out"))?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        state
            .auth
            .config
            .cookie(state.auth.config.session_cookie(), "", 0)
            .parse()
            .unwrap(),
    );
    Ok(response)
}
