use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use serde_json::Value;
use std::io::{self, IsTerminal, Write};

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

pub async fn resolve_vehicle(
    store: &Store,
    profile: &Profile,
    context: &VehicleRequestContext,
    vin: Option<String>,
) -> Result<String> {
    if let Some(vin) = vin {
        return store.resolve_vin(profile.id, Some(&vin));
    }

    if let Some(default_vin) = store.get_default_vin(profile.id)? {
        return Ok(default_vin);
    }

    let data = context
        .client
        .get_vehicle_list(&context.access_token)
        .await
        .map_err(|err| anyhow!("failed to fetch vehicle list: {err:#}"))?;
    let vins = extract_vehicle_vins(&data);

    match vins.len() {
        0 => {
            return Err(anyhow!(
                "no vehicles were returned by the vehicle list endpoint"
            ));
        }
        1 => {
            store.sync_vins(profile.id, &vins, vins.first().map(String::as_str))?;
            return Ok(vins[0].clone());
        }
        _ => {}
    }

    store.sync_vins(profile.id, &vins, None)?;
    let selected_vin = prompt_for_default_vin(&vins)?;
    store.sync_vins(profile.id, &vins, Some(&selected_vin))?;
    Ok(selected_vin)
}

pub fn vehicle_list_output(
    profile: &Profile,
    context: &VehicleRequestContext,
    data: Value,
) -> VehicleOutput {
    VehicleOutput {
        ok: true,
        profile: profile.name.clone(),
        base_url: context.client.base_url().to_owned(),
        data,
    }
}

pub fn extract_vehicle_vins(data: &Value) -> Vec<String> {
    let vehicles = data
        .pointer("/data/data")
        .and_then(Value::as_array)
        .or_else(|| data.pointer("/data").and_then(Value::as_array))
        .or_else(|| data.as_array());

    vehicles
        .into_iter()
        .flatten()
        .filter_map(|vehicle| vehicle.get("vin").and_then(Value::as_str))
        .map(str::trim)
        .filter(|vin| !vin.is_empty())
        .map(str::to_uppercase)
        .fold(Vec::new(), |mut vins, vin| {
            if !vins.contains(&vin) {
                vins.push(vin);
            }
            vins
        })
}

fn prompt_for_default_vin(vins: &[String]) -> Result<String> {
    let mut stderr = io::stderr();
    writeln!(
        stderr,
        "No default VIN configured. Select one of the available vehicles:"
    )?;
    for (index, vin) in vins.iter().enumerate() {
        writeln!(stderr, "  {}. {}", index + 1, vin)?;
    }

    if !io::stdin().is_terminal() {
        return Err(anyhow!(
            "multiple vehicles available and no interactive terminal is available; run `vehicle vin default --vin <VIN>`"
        ));
    }

    write!(stderr, "Select vehicle [1-{}]: ", vins.len())?;
    stderr.flush()?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read selected vehicle")?;
    let selection = input.trim();

    if let Ok(index) = selection.parse::<usize>()
        && let Some(vin) = vins.get(index.saturating_sub(1))
        && index > 0
    {
        return Ok(vin.clone());
    }

    if let Some(vin) = vins
        .iter()
        .find(|vin| vin.eq_ignore_ascii_case(selection))
        .cloned()
    {
        return Ok(vin);
    }

    Err(anyhow!(
        "invalid vehicle selection `{selection}`; choose a number from 1 to {} or enter a listed VIN",
        vins.len()
    ))
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

    let refreshed = if is_bridge_managed(&session) {
        VolvoClient::refresh_access_token_bridge(
            &reqwest::Client::new(),
            &session.token_endpoint,
            &refresh_token,
        )
        .await
    } else {
        VolvoClient::refresh_access_token(
            &reqwest::Client::new(),
            &session.token_endpoint,
            &session.client_id,
            &session.client_secret,
            &refresh_token,
        )
        .await
    }
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

fn is_bridge_managed(session: &AuthSession) -> bool {
    let token_endpoint = session.token_endpoint.to_ascii_lowercase();
    token_endpoint.contains("/v1/oauth/refresh")
}

#[cfg(test)]
mod tests {
    use super::{is_bridge_managed, resolve_api_key, should_refresh};
    use crate::store::sqlite::AuthSession;

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

    #[test]
    fn bridge_sessions_use_bridge_refresh_endpoint() {
        let session = AuthSession {
            profile_id: 1,
            access_token: "a".to_owned(),
            refresh_token: Some("r".to_owned()),
            scope: None,
            token_type: None,
            expires_at: None,
            token_endpoint: "https://bridge.example.com/v1/oauth/refresh".to_owned(),
            client_id: "bridge-managed".to_owned(),
            client_secret: "bridge-managed".to_owned(),
        };
        assert!(is_bridge_managed(&session));
    }
}
