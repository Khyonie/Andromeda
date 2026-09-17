use axum::{Json, extract::State, http::StatusCode};
use serde::Deserialize;
use sqlx::SqlitePool;
use virt::domain::Domain;

use crate::{
    database, libvirt,
    model::{Instance, VmConfig},
    seed::{self, disk},
    server::{
        ApiError, AppState,
        log::{OperationLog, Severity, SharedLogger},
    },
};

fn operation_error(
    operation: &OperationLog,
    status: StatusCode,
    error: impl std::fmt::Display,
) -> ApiError {
    let severity = if status.is_server_error() {
        Severity::ERROR
    } else {
        Severity::WARNING
    };
    operation.failure(severity, &error);
    ApiError {
        status,
        message: error.to_string(),
    }
}

async fn lookup_instance(
    database: &SqlitePool,
    id: &str,
    operation: &mut OperationLog,
) -> Result<Instance, ApiError> {
    operation.step("Looking up instance in database");
    database::get_domain_by_id(database, id)
        .await
        .map_err(|e| operation_error(operation, StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(|| operation_error(operation, StatusCode::NOT_FOUND, "No such instance"))
}

#[derive(Deserialize)]
pub struct GetInstanceRequest {
    id: String,
}

// GET /api/instance with a JSON body: {"id": "<instance UUID>"}
pub(super) async fn get_instance(
    State(database): State<SqlitePool>,
    State(logger): State<SharedLogger>,
    Json(request): Json<GetInstanceRequest>,
) -> Result<Json<Instance>, ApiError> {
    let mut operation = OperationLog::new(logger, "get", Some(&request.id));
    let instance = lookup_instance(&database, &request.id, &mut operation).await?;
    operation.info("Completed: returning instance data");
    Ok(Json(instance))
}

// GET /api/instance/ids
pub(super) async fn get_instance_ids(
    State(database): State<SqlitePool>,
    State(logger): State<SharedLogger>,
) -> Result<Json<Vec<String>>, ApiError> {
    let mut operation = OperationLog::new(logger, "list", None);
    operation.step("Reading instance IDs from database");
    let ids = database::get_instance_ids(&database)
        .await
        .map_err(|e| operation_error(&operation, StatusCode::INTERNAL_SERVER_ERROR, e))?;
    operation.info(format!("Completed: returning {} instance IDs", ids.len()));
    Ok(Json(ids))
}

// PUT /api/instance
pub(super) async fn create_instance(
    State(state): State<AppState>,
    Json(config): Json<VmConfig>,
) -> Result<StatusCode, ApiError> {
    let mut operation = OperationLog::new(
        state.logger.clone(),
        "create",
        Some(&config.instance.hostname),
    );
    if state.flags.dry_run {
        operation.info(
            "Dry run: disk commands, domain definition, and database insertion will be skipped",
        );
    }
    operation.step("Validating instance paths");
    let paths = state
        .paths
        .instance(&config.instance.hostname)
        .map_err(|e| operation_error(&operation, StatusCode::BAD_REQUEST, e))?;
    operation.step("Generating cloud-init data");
    let user_data = seed::user_seed(&config);
    let network_config = seed::network_config_seed(&config);
    let meta_data = seed::metadata_seed(&config);

    operation.step("Writing temporary seed files");
    let seeds = seed::SeedFiles::write(&user_data, &network_config, &meta_data, &operation)
        .map_err(|e| operation_error(&operation, StatusCode::INTERNAL_SERVER_ERROR, e))?;

    operation.step("Preparing instance disk");
    disk::create_image(&paths, config.instance.disk_size, &state.flags, &operation)
        .map_err(|e| operation_error(&operation, StatusCode::INTERNAL_SERVER_ERROR, e))?;

    operation.step("Preparing cloud-init ISO");
    disk::create_iso(&paths, &seeds, &state.flags, &operation)
        .map_err(|e| operation_error(&operation, StatusCode::INTERNAL_SERVER_ERROR, e))?;
    drop(seeds);

    operation.step("Generating domain XML");
    let (xml, id) = libvirt::generate_domain(&config, &paths)
        .map_err(|e| operation_error(&operation, StatusCode::INTERNAL_SERVER_ERROR, e))?;

    if state.flags.dry_run {
        operation.info(format!(
            "Completed dry run: would define and register domain with UUID {id}"
        ));
        return Ok(StatusCode::ACCEPTED);
    }

    operation.step("Defining libvirt domain");
    Domain::define_xml(&state.qemu, &xml)
        .map_err(|e| operation_error(&operation, StatusCode::INTERNAL_SERVER_ERROR, e))?;
    operation.info(format!("Defined domain with UUID {id}"));

    operation.step("Inserting instance into database");
    sqlx::query(
        r#"
        INSERT INTO instances (
            id, hostname, memory_mib, vcpus, mac_address,
            ipv4_address, remote_port, service_port
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(id.to_string())
    .bind(config.instance.hostname)
    .bind(config.instance.memory as u32)
    .bind(config.instance.vcpus)
    .bind(config.networking.mac)
    .bind(config.networking.ip)
    .bind(config.networking.remote_port)
    .bind(config.networking.service_port)
    .execute(&state.database)
    .await
    .map_err(|e| operation_error(&operation, StatusCode::INTERNAL_SERVER_ERROR, e))?;

    operation.info(format!("Completed: registered new domain with UUID {id}"));
    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
pub struct DeleteInstanceRequest {
    id: String,
}

// DELETE /api/instance
pub(super) async fn delete_instance(
    State(state): State<AppState>,
    Json(request): Json<DeleteInstanceRequest>,
) -> Result<StatusCode, ApiError> {
    let mut operation = OperationLog::new(state.logger.clone(), "delete", Some(&request.id));
    let instance = lookup_instance(&state.database, &request.id, &mut operation).await?;
    operation.step("Looking up libvirt domain");
    let domain = instance
        .get_domain(&state.qemu)
        .map_err(|e| operation_error(&operation, StatusCode::INTERNAL_SERVER_ERROR, e))?;

    operation.step("Checking whether domain is active");
    if domain
        .is_active()
        .map_err(|e| operation_error(&operation, StatusCode::INTERNAL_SERVER_ERROR, e))?
    {
        return Err(operation_error(
            &operation,
            StatusCode::CONFLICT,
            "Domain is currently active. Destroy it before attempting undefine.",
        ));
    }

    operation.step("Undefining libvirt domain");
    domain
        .undefine()
        .map_err(|e| operation_error(&operation, StatusCode::INTERNAL_SERVER_ERROR, e))?;

    operation.step("Deleting instance from database");
    database::delete_instance_by_id(&state.database, &request.id)
        .await
        .map_err(|e| operation_error(&operation, StatusCode::INTERNAL_SERVER_ERROR, e))?;

    operation.info("Completed: instance deleted");
    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
pub struct StartInstanceRequest {
    id: String,
}

// POST /api/instance/start
pub(super) async fn start_instance(
    State(state): State<AppState>,
    Json(request): Json<StartInstanceRequest>,
) -> Result<StatusCode, ApiError> {
    let mut operation = OperationLog::new(state.logger.clone(), "start", Some(&request.id));
    let instance = lookup_instance(&state.database, &request.id, &mut operation).await?;
    operation.step("Looking up libvirt domain");
    let domain = instance
        .get_domain(&state.qemu)
        .map_err(|e| operation_error(&operation, StatusCode::INTERNAL_SERVER_ERROR, e))?;
    operation.step("Starting libvirt domain");
    domain
        .create()
        .map_err(|e| operation_error(&operation, StatusCode::INTERNAL_SERVER_ERROR, e))?;
    operation.info("Completed: instance started");
    Ok(StatusCode::ACCEPTED)
}

// POST /api/instance/stop
pub(super) async fn stop_instance(
    State(state): State<AppState>,
    Json(request): Json<StartInstanceRequest>,
) -> Result<StatusCode, ApiError> {
    let mut operation = OperationLog::new(state.logger.clone(), "stop", Some(&request.id));
    let instance = lookup_instance(&state.database, &request.id, &mut operation).await?;
    operation.step("Looking up libvirt domain");
    let domain = instance
        .get_domain(&state.qemu)
        .map_err(|e| operation_error(&operation, StatusCode::INTERNAL_SERVER_ERROR, e))?;
    operation.step("Stopping libvirt domain");
    domain
        .destroy()
        .map_err(|e| operation_error(&operation, StatusCode::INTERNAL_SERVER_ERROR, e))?;
    operation.info("Completed: instance stopped");
    Ok(StatusCode::ACCEPTED)
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
    use crate::server::log::Logger;

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
            Json(GetInstanceRequest {
                id: "missing".into(),
            }),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        sqlx::migrate!("./migrations").run(&database).await.unwrap();
        let response = get_instance(
            State(database.clone()),
            State(logger.clone()),
            Json(GetInstanceRequest {
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
        assert!(entries[2].contains("ERROR"));
        assert!(entries[5].contains("WARNING"));
    }

    #[tokio::test]
    async fn list_instance_ids_returns_empty_or_sorted_json_and_reports_database_errors() {
        let database = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&database).await.unwrap();

        let response = get_instance_ids(State(database.clone()), State(Logger::shared()))
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
                "INSERT INTO instances (id, hostname, memory_mib, vcpus, mac_address, \
                 remote_port, service_port) VALUES (?, ?, 512, 1, ?, ?, ?)",
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
        let response = get_instance_ids(State(database.clone()), State(Logger::shared()))
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
        let response = get_instance_ids(State(database), State(Logger::shared()))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    async fn get_with_json(database: &SqlitePool, body: &str) -> Response {
        let request = Request::builder()
            .method("GET")
            .uri("/api/instance")
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))
            .unwrap();
        match Json::<GetInstanceRequest>::from_request(request, &()).await {
            Ok(request) => get_instance(State(database.clone()), State(Logger::shared()), request)
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
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO instances (id, hostname, memory_mib, vcpus, mac_address, \
             ipv4_address, remote_port, service_port) \
             VALUES (?, 'vm-test', 512, 2, '52:54:00:00:00:01', '10.0.0.2', 25566, 25565)",
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
                "hostname": "vm-test",
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
