use sqlx::SqlitePool;

use crate::model::{Instance, InstanceSummary, VmConfig};

pub async fn insert_instance(
    database: &SqlitePool,
    id: &uuid::Uuid,
    config: &VmConfig,
    owner_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO instances (
            id, hostname, memory_mib, vcpus, mac_address,
            ipv4_address, remote_port, service_port, owner_id, description
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(id.to_string())
    .bind(&config.instance.hostname)
    .bind(config.instance.memory as u32)
    .bind(config.instance.vcpus)
    .bind(&config.networking.mac)
    .bind(&config.networking.ip)
    .bind(config.networking.remote_port)
    .bind(config.networking.service_port)
    .bind(owner_id)
    .bind(&config.instance.description)
    .execute(database)
    .await?;
    Ok(())
}

pub async fn get_instance_by_id(
    database: &SqlitePool,
    id: &str,
) -> Result<Option<Instance>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT
            id,
            owner_id,
            hostname,
            description,
            memory_mib,
            vcpus,
            mac_address,
            ipv4_address,
            remote_port,
            service_port
        FROM instances
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(database)
    .await
}

pub async fn get_instance_ids(
    database: &SqlitePool,
    user_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT i.id FROM instances i WHERE i.owner_id = ? OR EXISTS (SELECT 1 FROM instance_members m WHERE m.instance_id = i.id AND m.user_id = ?) ORDER BY i.id")
        .bind(user_id).bind(user_id)
        .fetch_all(database)
        .await
}

pub async fn delete_instance_by_id(database: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM instances WHERE id = ?")
        .bind(id)
        .execute(database)
        .await?;
    Ok(())
}

pub async fn get_instance_summaries(
    database: &SqlitePool,
    user_id: &str,
) -> Result<Vec<InstanceSummary>, sqlx::Error> {
    sqlx::query_as(
        "SELECT i.id, i.hostname, i.description, i.remote_port, i.service_port FROM instances i \
         WHERE i.owner_id = ? OR EXISTS \
         (SELECT 1 FROM instance_members m WHERE m.instance_id = i.id AND m.user_id = ?) \
         ORDER BY i.hostname COLLATE NOCASE, i.id",
    )
    .bind(user_id)
    .bind(user_id)
    .fetch_all(database)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn deleting_an_instance_preserves_other_rows() {
        let database = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&database).await.unwrap();
        sqlx::query("INSERT INTO users (id, discord_id, username, display_name, created_at, updated_at) VALUES ('test-account', '123', 'test', 'Test', 0, 0)").execute(&database).await.unwrap();

        let first_id = uuid::Uuid::new_v4().to_string();
        let second_id = uuid::Uuid::new_v4().to_string();
        for (index, id) in [&first_id, &second_id].into_iter().enumerate() {
            sqlx::query(
                "INSERT INTO instances (owner_id, id, hostname, memory_mib, vcpus, mac_address, \
                 ipv4_address, remote_port, service_port) VALUES ('test-account', ?, ?, 512, 1, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(format!("vm-{index}"))
            .bind(format!("52:54:00:00:00:0{index}"))
            .bind(format!("10.0.0.{}", index + 2))
            .bind(25565 + index as i32)
            .bind(25565 + index as i32)
            .execute(&database)
            .await
            .unwrap();
        }

        assert!(
            get_instance_by_id(&database, &first_id)
                .await
                .unwrap()
                .is_some()
        );
        // IDs are bound as values, even when they contain SQL syntax.
        delete_instance_by_id(&database, "' OR 1=1 --")
            .await
            .unwrap();
        assert!(
            get_instance_by_id(&database, &first_id)
                .await
                .unwrap()
                .is_some()
        );

        delete_instance_by_id(&database, &first_id).await.unwrap();
        assert!(
            get_instance_by_id(&database, &first_id)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            get_instance_by_id(&database, &second_id)
                .await
                .unwrap()
                .is_some()
        );
        database.close().await;
    }

    #[tokio::test]
    async fn deleting_an_instance_propagates_database_errors() {
        let database = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        // An unmigrated database must return an error, not report success.
        assert!(delete_instance_by_id(&database, "missing").await.is_err());
        database.close().await;
    }
}
