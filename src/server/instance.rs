use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sqlx::SqlitePool;

use crate::{
    instances::{self, CreateOutcome},
    logging::SharedLogger,
    model::{InstanceListEntry, InstanceView, VmConfig},
};

use super::{auth::CurrentSession, error::ApiError, requests::InstanceRequest, state::AppState};

// GET /api/instance/id with a JSON body: {"id": "<instance UUID>"}
pub(super) async fn get_instance(
    State(database): State<SqlitePool>,
    State(logger): State<SharedLogger>,
    CurrentSession(session): CurrentSession,
    Json(request): Json<InstanceRequest>,
) -> Result<Json<InstanceView>, ApiError> {
    Ok(Json(
        instances::get_instance(&database, logger, &session.account, &request.id).await?,
    ))
}

// GET /api/instance
pub(super) async fn get_instance_ids(
    State(database): State<SqlitePool>,
    State(logger): State<SharedLogger>,
    CurrentSession(session): CurrentSession,
) -> Result<Json<Vec<String>>, ApiError> {
    Ok(Json(
        instances::get_instance_ids(&database, logger, &session.account).await?,
    ))
}

// GET /api/instance/summary
pub(super) async fn list_instances(
    State(state): State<AppState>,
    CurrentSession(session): CurrentSession,
) -> Result<Json<Vec<InstanceListEntry>>, ApiError> {
    Ok(Json(state.instances.list(&session.account).await?))
}

// PUT /api/instance
pub(super) async fn create_instance(
    State(state): State<AppState>,
    CurrentSession(session): CurrentSession,
    Json(config): Json<VmConfig>,
) -> Result<Response, ApiError> {
    match state.instances.create(&session.account, config).await? {
        CreateOutcome::Created { id } => {
            Ok((StatusCode::OK, Json(serde_json::json!({ "id": id }))).into_response())
        }
        CreateOutcome::DryRun => Ok(StatusCode::ACCEPTED.into_response()),
    }
}

// DELETE /api/instance
pub(super) async fn delete_instance(
    State(state): State<AppState>,
    CurrentSession(session): CurrentSession,
    Json(request): Json<InstanceRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .instances
        .delete(&session.account, &request.id)
        .await?;
    Ok(StatusCode::OK)
}

// POST /api/instance/start
pub(super) async fn start_instance(
    State(state): State<AppState>,
    CurrentSession(session): CurrentSession,
    Json(request): Json<InstanceRequest>,
) -> Result<StatusCode, ApiError> {
    state.instances.start(&session.account, &request.id).await?;
    Ok(StatusCode::ACCEPTED)
}

// POST /api/instance/stop
pub(super) async fn stop_instance(
    State(state): State<AppState>,
    CurrentSession(session): CurrentSession,
    Json(request): Json<InstanceRequest>,
) -> Result<StatusCode, ApiError> {
    state.instances.stop(&session.account, &request.id).await?;
    Ok(StatusCode::ACCEPTED)
}

// DELETE /api/instance/stop
pub(super) async fn destroy_instance(
    State(state): State<AppState>,
    CurrentSession(session): CurrentSession,
    Json(request): Json<InstanceRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .instances
        .destroy(&session.account, &request.id)
        .await?;
    Ok(StatusCode::ACCEPTED)
}

// POST /api/instance/status with {"id":"<UUID>"}.
pub(super) async fn get_status(
    State(state): State<AppState>,
    CurrentSession(session): CurrentSession,
    Json(request): Json<InstanceRequest>,
) -> Result<Json<crate::libvirt::metrics::InstanceStatus>, ApiError> {
    Ok(Json(
        state
            .instances
            .status(&session.account, &request.id)
            .await?,
    ))
}

// POST /api/instance/disk returns a streamed, standalone qcow2 attachment.
pub(super) async fn download_disk(
    State(state): State<AppState>,
    CurrentSession(session): CurrentSession,
    Json(request): Json<InstanceRequest>,
) -> Result<Response, ApiError> {
    use tokio::io::AsyncReadExt;
    let export = state
        .instances
        .download_disk(&session.account, &request.id)
        .await?;
    let file = export.file.try_clone().map_err(|error| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: error.to_string(),
    })?;
    let size = export.size;
    // The workspace stays alive with the stream and is removed when consumed or dropped.
    let stream = futures_util::stream::try_unfold(
        (tokio::fs::File::from_std(file), export),
        |(mut file, export)| async move {
            let mut bytes = vec![0; 64 * 1024];
            let read = file.read(&mut bytes).await?;
            if read == 0 {
                return Ok::<_, std::io::Error>(None);
            }
            bytes.truncate(read);
            Ok(Some((bytes, (file, export))))
        },
    );
    let mut response = Response::new(axum::body::Body::from_stream(stream));
    let headers = response.headers_mut();
    headers.insert("content-type", "application/octet-stream".parse().unwrap());
    // A constant filename avoids reflecting untrusted input into a response header.
    headers.insert(
        "content-disposition",
        "attachment; filename=instance.qcow2".parse().unwrap(),
    );
    headers.insert("content-length", size.into());
    headers.insert("cache-control", "no-store".parse().unwrap());
    Ok(response)
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        extract::FromRequest,
        http::{Request, header::CONTENT_TYPE},
        response::{IntoResponse, Response},
    };
    use serde_json::json;
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;
    use crate::logging::Logger;

    fn session() -> CurrentSession {
        CurrentSession(crate::auth::Session {
            account: crate::auth::Account {
                id: "test-account".into(),
                discord_id: "123".into(),
                username: "test".into(),
                display_name: "Test".into(),
                avatar: None,
            },
            csrf_token: "test-csrf".into(),
            token_hash: "test-session".into(),
        })
    }

    #[tokio::test]
    async fn lookup_failures_log_the_step_without_reporting_completion() {
        let database = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let logger = Logger::shared();
        let response = get_instance(
            State(database.clone()),
            State(logger.clone()),
            session(),
            Json(InstanceRequest {
                id: "missing".into(),
            }),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        sqlx::migrate!("./migrations").run(&database).await.unwrap();
        sqlx::query("INSERT INTO users (id, discord_id, username, display_name, created_at, updated_at) VALUES ('test-account', '123', 'test', 'Test', 0, 0)").execute(&database).await.unwrap();
        let response = get_instance(
            State(database.clone()),
            State(logger.clone()),
            session(),
            Json(InstanceRequest {
                id: "missing".into(),
            }),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        database.close().await;

        let logger = logger.lock().unwrap();
        let entries = logger.entries();
        assert_eq!(entries.len(), 6);
        for request in entries.as_chunks::<3>().0 {
            assert!(request[0].contains("Request received"));
            assert!(request[1].contains("Looking up instance in database"));
            assert!(request[2].contains("Looking up instance in database failed:"));
            assert!(request.iter().all(|entry| !entry.contains("Completed")));
        }
        assert!(entries[2].contains("[ Error ]"));
        assert!(entries[5].contains("[ Warning ]"));
    }

    #[tokio::test]
    async fn list_instance_ids_returns_empty_or_sorted_json_and_reports_database_errors() {
        let database = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&database).await.unwrap();
        sqlx::query("INSERT INTO users (id, discord_id, username, display_name, created_at, updated_at) VALUES ('test-account', '123', 'test', 'Test', 0, 0)").execute(&database).await.unwrap();

        let response =
            get_instance_ids(State(database.clone()), State(Logger::shared()), session())
                .await
                .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        assert_eq!(&bytes[..], b"[]");

        let ids = [
            "00000000-0000-4000-8000-000000000002",
            "00000000-0000-4000-8000-000000000001",
        ];
        for (index, id) in ids.iter().enumerate() {
            sqlx::query(
                "INSERT INTO instances (owner_id, id, hostname, memory_mib, vcpus, mac_address, \
                 remote_port, service_port) VALUES ('test-account', ?, ?, 512, 1, ?, ?, ?)",
            )
            .bind(id)
            .bind(format!("vm-{index}"))
            .bind(format!("52:54:00:00:00:0{index}"))
            .bind(25565 + index as i32)
            .bind(25565 + index as i32)
            .execute(&database)
            .await
            .unwrap();
        }
        let response =
            get_instance_ids(State(database.clone()), State(Logger::shared()), session())
                .await
                .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[CONTENT_TYPE], "application/json");
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
            json!([ids[1], ids[0]])
        );

        database.close().await;
        let response = get_instance_ids(State(database), State(Logger::shared()), session())
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    async fn get_with_json(database: &SqlitePool, body: &str) -> Response {
        let request = Request::builder()
            .method("GET")
            .uri("/api/instance/id")
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))
            .unwrap();
        match Json::<InstanceRequest>::from_request(request, &()).await {
            Ok(request) => get_instance(
                State(database.clone()),
                State(Logger::shared()),
                session(),
                request,
            )
            .await
            .into_response(),
            Err(rejection) => rejection.into_response(),
        }
    }

    #[tokio::test]
    async fn get_instance_reads_json_id_and_returns_database_fields() {
        let database = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&database).await.unwrap();
        sqlx::query("INSERT INTO users (id, discord_id, username, display_name, created_at, updated_at) VALUES ('test-account', '123', 'test', 'Test', 0, 0)").execute(&database).await.unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO instances (owner_id, id, hostname, memory_mib, vcpus, mac_address, \
             ipv4_address, remote_port, service_port) \
             VALUES ('test-account', ?, 'vm-test', 512, 2, '52:54:00:00:00:01', '10.0.0.2', 25566, 25565)",
        )
        .bind(&id)
        .execute(&database)
        .await
        .unwrap();

        let body = json!({"id": id}).to_string();
        let response = get_with_json(&database, &body).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[CONTENT_TYPE], "application/json");
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
            json!({
                "id": id,
                "owner_id": "test-account",
                "role": "owner",
                "permissions": { "view": true, "control": true, "download": true, "delete": true, "manage_access": true },
                "hostname": "vm-test",
                "description": "",
                "memory_mib": 512,
                "vcpus": 2,
                "mac_address": "52:54:00:00:00:01",
                "ipv4_address": "10.0.0.2",
                "remote_port": 25566,
                "service_port": 25565,
            })
        );
        assert_eq!(
            get_with_json(&database, r#"{"id":"missing"}"#)
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            get_with_json(&database, "{}").await.status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        database.close().await;
        assert_eq!(
            get_with_json(&database, &body).await.status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
