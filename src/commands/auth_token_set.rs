use anyhow::{Result, anyhow};
use serde::Serialize;

use crate::config::DEFAULT_AUTH_ISSUER;
use crate::store::sqlite::{PersistedTokenSet, Profile, Store, unix_now};

#[derive(Debug, Clone)]
pub struct AuthTokenSetArgs {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    pub scope: Option<String>,
    pub token_type: Option<String>,
    pub token_endpoint: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuthTokenSetOutput {
    pub ok: bool,
    pub profile: String,
    pub base_url: String,
    pub expires_at: Option<i64>,
    pub scope: Option<String>,
    pub token_type: Option<String>,
    pub refresh_token_present: bool,
}

pub fn execute(
    store: &Store,
    profile: &Profile,
    base_url: &str,
    args: AuthTokenSetArgs,
) -> Result<AuthTokenSetOutput> {
    let access_token = normalize_required(args.access_token, "access token")?;
    let refresh_token = normalize_optional(args.refresh_token);
    let token_endpoint = resolve_token_endpoint(args.token_endpoint);
    let client_id =
        normalize_optional(args.client_id).unwrap_or_else(|| "manual-token-set".to_owned());
    let client_secret =
        normalize_optional(args.client_secret).unwrap_or_else(|| "manual-token-set".to_owned());
    let scope = normalize_optional(args.scope);
    let token_type = normalize_optional(args.token_type);
    let expires_at = args.expires_in.map(|seconds| unix_now() + seconds as i64);

    store.save_auth_session(
        profile.id,
        &PersistedTokenSet {
            access_token,
            refresh_token: refresh_token.clone(),
            scope: scope.clone(),
            token_type: token_type.clone(),
            expires_at,
            token_endpoint,
            client_id,
            client_secret,
        },
    )?;

    Ok(AuthTokenSetOutput {
        ok: true,
        profile: profile.name.clone(),
        base_url: base_url.to_owned(),
        expires_at,
        scope,
        token_type,
        refresh_token_present: refresh_token.is_some(),
    })
}

fn normalize_required(value: String, field: &str) -> Result<String> {
    let trimmed = value.trim().to_owned();
    if trimmed.is_empty() {
        return Err(anyhow!("{field} cannot be empty"));
    }
    Ok(trimmed)
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let t = v.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_owned())
        }
    })
}

fn resolve_token_endpoint(value: Option<String>) -> String {
    let default = format!("{}/oauth2/token", DEFAULT_AUTH_ISSUER);
    normalize_optional(value).unwrap_or(default)
}
