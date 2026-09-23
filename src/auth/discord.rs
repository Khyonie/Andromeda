use anyhow::{Result, ensure};
use curl::easy::{Easy, List};
use serde::Deserialize;
use std::time::Duration;

use super::AuthConfig;

#[derive(Deserialize)]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
}

#[derive(Deserialize)]
struct Token {
    access_token: String,
}

// Run on the blocking pool. Tokens and upstream response bodies are never logged.
pub fn identify(config: &AuthConfig, code: &str) -> Result<DiscordUser> {
    let form = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("client_id", config.client_id.as_deref().unwrap_or_default())
        .append_pair(
            "client_secret",
            config.client_secret.as_deref().unwrap_or_default(),
        )
        .append_pair("grant_type", "authorization_code")
        .append_pair("code", code)
        .append_pair("redirect_uri", &config.callback_url())
        .finish();
    let token: Token = serde_json::from_slice(&request(
        "https://discord.com/api/oauth2/token",
        Some(&form),
        None,
    )?)?;
    let user: DiscordUser = serde_json::from_slice(&request(
        "https://discord.com/api/v10/users/@me",
        None,
        Some(&token.access_token),
    )?)?;
    ensure!(
        !user.id.is_empty() && user.id.bytes().all(|b| b.is_ascii_digit()),
        "Invalid Discord identity"
    );
    Ok(user)
}

fn request(url: &str, form: Option<&str>, bearer: Option<&str>) -> Result<Vec<u8>> {
    let mut client = Easy::new();
    client.url(url)?;
    client.timeout(Duration::from_secs(15))?;
    client.connect_timeout(Duration::from_secs(5))?;
    client.useragent("Andromeda/0.1")?;
    let mut headers = List::new();
    headers.append("Accept: application/json")?;
    if let Some(form) = form {
        headers.append("Content-Type: application/x-www-form-urlencoded")?;
        client.post_fields_copy(form.as_bytes())?;
    }
    if let Some(bearer) = bearer {
        headers.append(&format!("Authorization: Bearer {bearer}"))?;
    }
    client.http_headers(headers)?;
    let mut bytes = Vec::new();
    {
        let mut transfer = client.transfer();
        transfer.write_function(|chunk| {
            if bytes.len() + chunk.len() > 65536 {
                return Ok(0);
            }
            bytes.extend_from_slice(chunk);
            Ok(chunk.len())
        })?;
        transfer.perform()?;
    }
    ensure!(
        client.response_code()? == 200,
        "Discord authentication request failed"
    );
    Ok(bytes)
}
