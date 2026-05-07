use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use serde_json::Value;

use crate::http::client::VolvoClient;
use crate::store::sqlite::{AuthSession, PersistedTokenSet, Profile, Store, unix_now};

#[derive(Debug, Clone)]
pub struct VehicleApiArgs {
    pub api_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VehicleVinApiArgs {
    pub vin: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Clone)]
pub struct VehicleRequestContext {
    pub client: VolvoClient,
    pub access_token: String,
}

#[derive(Debug, Serialize)]
pub struct VehicleOutput {
    pub ok: bool,
    pub profile: String,
    pub base_url: String,
    pub data: Value,
}

#[derive(Debug, Serialize)]
pub struct VehicleVinOutput {
    pub ok: bool,
    pub profile: String,
    pub base_url: String,
    pub vin: String,
    pub data: Value,
}

pub async fn build_request_context(
    store: &Store,
    profile: &Profile,
    base_url: &str,
    api_key_override: Option<String>,
) -> Result<VehicleRequestContext> {
    let api_key = resolve_api_key(api_key_override)?;
    let session = ensure_fresh_session(store, profile).await?;
    let client = VolvoClient::new(base_url, &api_key)?;
    Ok(VehicleRequestContext {
        client,
        access_token: session.access_token,
    })
}

pub fn resolve_vin(store: &Store, profile: &Profile, vin: Option<String>) -> Result<String> {
    store.resolve_vin(profile.id, vin.as_deref())
}

pub fn resolve_api_key(cli_override: Option<String>) -> Result<String> {
    if let Some(value) = cli_override {
        let normalized = value.trim().to_owned();
        if !normalized.is_empty() {
            return Ok(normalized);
        }
    }

    let from_env = std::env::var("VCC_API_KEY").unwrap_or_default();
    let normalized = from_env.trim().to_owned();
    if normalized.is_empty() {
        return Err(anyhow!(
            "missing API key; provide `--api-key` or set `VCC_API_KEY`"
        ));
    }
    Ok(normalized)
}

pub fn should_refresh(expires_at: Option<i64>) -> bool {
    match expires_at {
        Some(deadline) => deadline <= unix_now() + 30,
        None => false,
    }
}

async fn ensure_fresh_session(store: &Store, profile: &Profile) -> Result<AuthSession> {
    let session = store
        .get_auth_session(profile.id)?
        .ok_or_else(|| anyhow!("no auth session; run `vc-cli auth login` first"))?;
    if !should_refresh(session.expires_at) {
        return Ok(session);
    }

    let refresh_token = session
        .refresh_token
        .clone()
        .ok_or_else(|| anyhow!("access token expired and no refresh token is available"))?;

    let refreshed = VolvoClient::refresh_access_token(
        &reqwest::Client::new(),
        &session.token_endpoint,
        &session.client_id,
        &session.client_secret,
        &refresh_token,
    )
    .await
    .context("failed to refresh access token")?;

    let expires_at = refreshed.expires_in.map(|value| unix_now() + value as i64);
    let new_refresh_token = refreshed.refresh_token.or(Some(refresh_token));
    store.save_auth_session(
        profile.id,
        &PersistedTokenSet {
            access_token: refreshed.access_token,
            refresh_token: new_refresh_token,
            scope: refreshed.scope,
            token_type: refreshed.token_type,
            expires_at,
            token_endpoint: session.token_endpoint,
            client_id: session.client_id,
            client_secret: session.client_secret,
        },
    )?;

    store
        .get_auth_session(profile.id)?
        .ok_or_else(|| anyhow!("missing auth session after token refresh"))
}

#[cfg(test)]
mod tests {
    use super::{resolve_api_key, should_refresh};

    #[test]
    fn refresh_when_expiring_soon() {
        let now = crate::store::sqlite::unix_now();
        assert!(should_refresh(Some(now + 10)));
        assert!(!should_refresh(Some(now + 120)));
    }

    #[test]
    fn api_key_override_wins() {
        let resolved = resolve_api_key(Some("  cli-key  ".to_owned())).expect("should resolve");
        assert_eq!(resolved, "cli-key");
    }
}
