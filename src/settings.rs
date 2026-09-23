//! Persistent defaults, read together once for each new instance.
use std::{net::Ipv4Addr, process::Stdio, time::Duration};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::{io::AsyncWriteExt, process::Command};

use crate::{
    instances::{ErrorKind, InstanceError},
    logging::{self, Severity, SharedLogger},
};

#[derive(Clone, Serialize)]
pub struct GuestDefaults {
    pub gateway_ip: Ipv4Addr,
    pub sysadmin_ssh_key: String,
    pub revision: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateDefaults {
    pub gateway_ip: String,
    pub sysadmin_ssh_key: String,
    pub revision: i64,
}

#[derive(Clone)]
pub struct SettingsService {
    database: SqlitePool,
    logger: SharedLogger,
}

fn invalid(message: &str) -> InstanceError {
    InstanceError {
        kind: ErrorKind::InvalidInput,
        message: message.into(),
    }
}

fn internal(message: &str) -> InstanceError {
    InstanceError {
        kind: ErrorKind::Internal,
        message: message.into(),
    }
}

impl SettingsService {
    pub async fn initialize(
        database: SqlitePool,
        gateway_ip: Ipv4Addr,
        logger: SharedLogger,
    ) -> Result<Self> {
        sqlx::query(
            "INSERT INTO system_settings (id, gateway_ip) VALUES (1, ?) ON CONFLICT(id) DO NOTHING",
        )
        .bind(gateway_ip.to_string())
        .execute(&database)
        .await?;
        Ok(Self { database, logger })
    }

    pub async fn snapshot(&self) -> Result<GuestDefaults> {
        let (gateway, sysadmin_ssh_key, revision): (String, String, i64) = sqlx::query_as(
            "SELECT gateway_ip, sysadmin_ssh_key, revision FROM system_settings WHERE id = 1",
        )
        .fetch_one(&self.database)
        .await?;
        Ok(GuestDefaults {
            gateway_ip: gateway.parse().context("Invalid stored guest gateway")?,
            sysadmin_ssh_key,
            revision,
        })
    }

    pub async fn update(
        &self,
        input: UpdateDefaults,
        actor_id: &str,
    ) -> Result<GuestDefaults, InstanceError> {
        logging::log_message(&self.logger, Severity::Info, "Validating guest defaults");
        let gateway_ip: Ipv4Addr = input
            .gateway_ip
            .trim()
            .parse()
            .map_err(|_| invalid("Gateway must be an IPv4 address, for example 10.0.0.1."))?;
        let key = input.sysadmin_ssh_key.trim();
        validate_public_key(key).await?;
        logging::log_message(&self.logger, Severity::Info, "Saving guest defaults");
        let revision: Option<i64> = sqlx::query_scalar(
            "UPDATE system_settings SET gateway_ip = ?, sysadmin_ssh_key = ?, revision = revision + 1 \
             WHERE id = 1 AND revision = ? RETURNING revision"
        ).bind(gateway_ip.to_string()).bind(key).bind(input.revision)
            .fetch_optional(&self.database).await.map_err(|_| internal("Could not save guest defaults."))?;
        let revision = revision.ok_or_else(|| InstanceError {
            kind: ErrorKind::Conflict,
            message:
                "These settings changed since you opened the page. Reload them before saving again."
                    .into(),
        })?;
        logging::log_message(
            &self.logger,
            Severity::Info,
            format!("Guest defaults saved by account {actor_id} (revision {revision})"),
        );
        Ok(GuestDefaults {
            gateway_ip,
            sysadmin_ssh_key: key.into(),
            revision,
        })
    }
}

async fn validate_public_key(key: &str) -> Result<(), InstanceError> {
    // An empty key disables the guest sysadmin account. Reject private keys,
    // multiple keys, and authorized_keys options before invoking OpenSSH.
    if key.is_empty() {
        return Ok(());
    }
    let kind = key.split_whitespace().next().unwrap_or_default();
    if key.len() > 16_384
        || key.chars().any(char::is_control)
        || !matches!(
            kind,
            "ssh-ed25519"
                | "ssh-rsa"
                | "ecdsa-sha2-nistp256"
                | "ecdsa-sha2-nistp384"
                | "ecdsa-sha2-nistp521"
                | "sk-ssh-ed25519@openssh.com"
                | "sk-ecdsa-sha2-nistp256@openssh.com"
        )
    {
        return Err(invalid(
            "Enter one OpenSSH public key (not a private key), or leave it empty.",
        ));
    }
    // Pass the key on stdin, never in command arguments, logs, or temporary files.
    let valid = tokio::time::timeout(Duration::from_secs(5), async {
        let mut child = Command::new("ssh-keygen").args(["-l", "-f", "/dev/stdin"])
            .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null())
            .kill_on_drop(true).spawn()?;
        let mut stdin = child.stdin.take().expect("piped stdin");
        stdin.write_all(key.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        drop(stdin);
        child.wait().await.map(|status| status.success())
    }).await.map_err(|_| internal("SSH key validation timed out. Please try again."))?
        .map_err(|_| internal("Could not validate the SSH key. Check that ssh-keygen is installed on the backend host."))?;
    if !valid {
        return Err(invalid(
            "The SSH public key is invalid. Paste the complete contents of your .pub file.",
        ));
    }
    Ok(())
}
