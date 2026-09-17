use std::{path::Path, time::Duration};

use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};

use crate::model::Instance;

pub async fn connect(path: impl AsRef<Path>) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

pub async fn get_domain_by_id(
    database: &SqlitePool,
    id: &str,
) -> Result<Option<Instance>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT
            id,
            hostname,
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

pub async fn get_instance_ids(database: &SqlitePool) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT id FROM instances ORDER BY id")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn deleting_an_instance_preserves_other_rows() {
        let database = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&database).await.unwrap();

        let first_id = uuid::Uuid::new_v4().to_string();
        let second_id = uuid::Uuid::new_v4().to_string();
        for (index, id) in [&first_id, &second_id].into_iter().enumerate() {
            sqlx::query(
                "INSERT INTO instances (id, hostname, memory_mib, vcpus, mac_address, \
                 ipv4_address, remote_port, service_port) VALUES (?, ?, 512, 1, ?, ?, ?, ?)",
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
            get_domain_by_id(&database, &first_id)
                .await
                .unwrap()
                .is_some()
        );
        // IDs are bound as values, even when they contain SQL syntax.
        delete_instance_by_id(&database, "' OR 1=1 --")
            .await
            .unwrap();
        assert!(
            get_domain_by_id(&database, &first_id)
                .await
                .unwrap()
                .is_some()
        );

        delete_instance_by_id(&database, &first_id).await.unwrap();
        assert!(
            get_domain_by_id(&database, &first_id)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            get_domain_by_id(&database, &second_id)
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
