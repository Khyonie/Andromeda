//! Local accounts and revocable sessions. Discord is used only to establish identity.
use crate::logging::{OperationLog, Severity, SharedLogger};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, SqlitePool};
use std::{collections::HashSet, env};

mod discord;
pub mod permissions;

const SESSION_SECONDS: i64 = 7 * 24 * 60 * 60;
const IDLE_SECONDS: i64 = 24 * 60 * 60;
const LOGIN_SECONDS: i64 = 10 * 60;

#[derive(Clone)]
pub struct AuthConfig {
    pub origin: String,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub secure: bool,
    admin_discord_ids: HashSet<String>,
}

impl AuthConfig {
    pub fn from_env() -> Result<Self> {
        let origin =
            env::var("ANDROMEDA_PUBLIC_ORIGIN").unwrap_or_else(|_| "http://localhost:5173".into());
        let url = url::Url::parse(&origin).context("Invalid ANDROMEDA_PUBLIC_ORIGIN")?;
        let secure = url.scheme() == "https";
        let allow_lan_http =
            env::var("ANDROMEDA_ALLOW_INSECURE_HTTP").is_ok_and(|value| value == "true");
        let local_host = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        let private_ip = match url.host() {
            Some(url::Host::Ipv4(address)) => address.is_private(),
            Some(url::Host::Ipv6(address)) => address.is_unique_local(),
            _ => false,
        };
        ensure!(
            secure || (url.scheme() == "http" && (local_host || (allow_lan_http && private_ip))),
            "Use HTTPS for ANDROMEDA_PUBLIC_ORIGIN. For development on a private LAN IP, set ANDROMEDA_ALLOW_INSECURE_HTTP=true"
        );
        ensure!(
            url.username().is_empty()
                && url.password().is_none()
                && url.path() == "/"
                && url.query().is_none()
                && url.fragment().is_none(),
            "ANDROMEDA_PUBLIC_ORIGIN must be an origin without a path or credentials"
        );
        let client_id = env::var("DISCORD_CLIENT_ID").ok().filter(|s| !s.is_empty());
        let client_secret = env::var("DISCORD_CLIENT_SECRET")
            .ok()
            .filter(|s| !s.is_empty());
        ensure!(
            client_id.is_some() == client_secret.is_some(),
            "Set both DISCORD_CLIENT_ID and DISCORD_CLIENT_SECRET"
        );
        let configured_admins = match env::var("ANDROMEDA_ADMIN_DISCORD_IDS") {
            Ok(value) => value,
            Err(env::VarError::NotPresent) => String::new(),
            Err(error) => return Err(error).context("Invalid ANDROMEDA_ADMIN_DISCORD_IDS"),
        };
        let mut admin_discord_ids = HashSet::new();
        if !configured_admins.trim().is_empty() {
            for id in configured_admins.split(',').map(str::trim) {
                ensure!(
                    id.parse::<u64>()
                        .is_ok_and(|value| value > 0 && value.to_string() == id),
                    "ANDROMEDA_ADMIN_DISCORD_IDS must be comma-separated numeric Discord user IDs"
                );
                admin_discord_ids.insert(id.to_owned());
            }
        }
        Ok(Self {
            origin: url.origin().ascii_serialization(),
            client_id,
            client_secret,
            secure,
            admin_discord_ids,
        })
    }
    pub fn is_admin(&self, account: &Account) -> bool {
        self.admin_discord_ids.contains(&account.discord_id)
    }
    pub fn enabled(&self) -> bool {
        self.client_id.is_some() && self.client_secret.is_some()
    }
    pub fn callback_url(&self) -> String {
        format!("{}/api/auth/discord/callback", self.origin)
    }
    pub fn session_cookie(&self) -> &'static str {
        if self.secure {
            "__Host-andromeda_session"
        } else {
            "andromeda_session"
        }
    }
    pub fn login_cookie(&self) -> &'static str {
        if self.secure {
            "__Host-andromeda_login"
        } else {
            "andromeda_login"
        }
    }
    pub fn cookie(&self, name: &str, value: &str, age: i64) -> String {
        format!(
            "{name}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={age}{}",
            if self.secure { "; Secure" } else { "" }
        )
    }
}

#[derive(Clone, Serialize, FromRow)]
pub struct Account {
    pub id: String,
    pub discord_id: String,
    pub username: String,
    pub display_name: String,
    pub avatar: Option<String>,
}

#[derive(Clone)]
pub struct Session {
    pub account: Account,
    pub csrf_token: String,
    pub token_hash: String,
}

#[derive(Clone)]
pub struct AuthService {
    database: SqlitePool,
    pub config: AuthConfig,
    logger: SharedLogger,
}

pub fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn random_token() -> Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|_| anyhow::anyhow!("Could not generate authentication token"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

impl AuthService {
    pub fn new(database: SqlitePool, config: AuthConfig, logger: SharedLogger) -> Self {
        Self {
            database,
            config,
            logger,
        }
    }

    pub async fn begin_login(&self, old_browser: Option<&str>) -> Result<(String, String)> {
        ensure!(self.config.enabled(), "Discord sign-in is not configured");
        let mut operation = OperationLog::new(self.logger.clone(), "sign in", None);
        operation.step("Creating Discord login request");
        let state = random_token()?;
        let browser = random_token()?;
        sqlx::query("DELETE FROM oauth_states WHERE expires_at <= ? OR browser_hash = ?")
            .bind(now())
            .bind(hash(old_browser.unwrap_or_default()))
            .execute(&self.database)
            .await?;
        sqlx::query("DELETE FROM sessions WHERE expires_at <= ? OR last_seen_at <= ?")
            .bind(now())
            .bind(now() - IDLE_SECONDS)
            .execute(&self.database)
            .await?;
        sqlx::query(
            "INSERT INTO oauth_states (state_hash, browser_hash, expires_at) VALUES (?, ?, ?)",
        )
        .bind(hash(&state))
        .bind(hash(&browser))
        .bind(now() + LOGIN_SECONDS)
        .execute(&self.database)
        .await?;
        let mut url = url::Url::parse("https://discord.com/oauth2/authorize")?;
        url.query_pairs_mut()
            .append_pair(
                "client_id",
                self.config.client_id.as_deref().unwrap_or_default(),
            )
            .append_pair("response_type", "code")
            .append_pair("scope", "identify")
            .append_pair("redirect_uri", &self.config.callback_url())
            .append_pair("state", &state);
        operation.info("Redirecting to Discord");
        Ok((
            url.to_string(),
            self.config
                .cookie(self.config.login_cookie(), &browser, LOGIN_SECONDS),
        ))
    }

    pub async fn consume_login(&self, state: &str, browser: &str) -> Result<bool> {
        if state.len() != 64 || browser.len() != 64 {
            return Ok(false);
        }
        Ok(sqlx::query(
            "DELETE FROM oauth_states WHERE state_hash = ? AND browser_hash = ? AND expires_at > ?",
        )
        .bind(hash(state))
        .bind(hash(browser))
        .bind(now())
        .execute(&self.database)
        .await?
        .rows_affected()
            == 1)
    }

    pub async fn finish_login(&self, code: String, old_token: Option<&str>) -> Result<String> {
        let mut operation = OperationLog::new(self.logger.clone(), "complete sign in", None);
        operation.step("Verifying Discord identity");
        let config = self.config.clone();
        let user = tokio::task::spawn_blocking(move || discord::identify(&config, &code)).await??;
        operation.step("Saving account and creating session");
        let token = random_token()?;
        let csrf = random_token()?;
        let mut tx = self.database.begin().await?;
        let id: String = sqlx::query_scalar("INSERT INTO users (id, discord_id, username, display_name, avatar, created_at, updated_at) \
            VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(discord_id) DO UPDATE SET username = excluded.username, \
            display_name = excluded.display_name, avatar = excluded.avatar, updated_at = excluded.updated_at RETURNING id")
            .bind(uuid::Uuid::new_v4().to_string()).bind(&user.id).bind(&user.username)
            .bind(user.global_name.as_deref().unwrap_or(&user.username)).bind(user.avatar).bind(now()).bind(now())
            .fetch_one(&mut *tx).await?;
        if let Some(old) = old_token {
            sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
                .bind(hash(old))
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("INSERT INTO sessions (token_hash, user_id, csrf_token, expires_at, last_seen_at) VALUES (?, ?, ?, ?, ?)")
            .bind(hash(&token)).bind(id).bind(csrf).bind(now()+SESSION_SECONDS).bind(now()).execute(&mut *tx).await?;
        tx.commit().await?;
        operation.info("Completed: signed in");
        Ok(self
            .config
            .cookie(self.config.session_cookie(), &token, SESSION_SECONDS))
    }

    pub async fn session(&self, token: Option<&str>) -> Result<Option<Session>> {
        let Some(token) =
            token.filter(|t| t.len() == 64 && t.bytes().all(|b| b.is_ascii_hexdigit()))
        else {
            return Ok(None);
        };
        let token_hash = hash(token);
        let row: Option<(String, String)> = sqlx::query_as("UPDATE sessions SET last_seen_at = ? WHERE token_hash = ? AND expires_at > ? AND last_seen_at > ? RETURNING user_id, csrf_token")
            .bind(now()).bind(&token_hash).bind(now()).bind(now()-IDLE_SECONDS).fetch_optional(&self.database).await?;
        let Some((id, csrf_token)) = row else {
            return Ok(None);
        };
        let account = sqlx::query_as::<_, Account>(
            "SELECT id, discord_id, username, display_name, avatar FROM users WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.database)
        .await?;
        Ok(account.map(|account| Session {
            account,
            csrf_token,
            token_hash,
        }))
    }

    pub async fn logout(&self, session: &Session) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(&session.token_hash)
            .execute(&self.database)
            .await?;
        crate::logging::log_message(&self.logger, Severity::Info, "Completed: signed out");
        Ok(())
    }

    pub fn failure(&self) {
        crate::logging::log_message(
            &self.logger,
            Severity::Warning,
            "Discord sign-in failed; no session issued",
        );
    }
}
