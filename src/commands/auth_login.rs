use anyhow::{Context, Result, anyhow};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::Rng;
use reqwest::Client;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{Duration, timeout};
use url::Url;
use url::form_urlencoded::Serializer;

use crate::config::{DEFAULT_AUTH_ISSUER, DEFAULT_REDIRECT_URI, DEFAULT_SCOPES};
use crate::http::client::VolvoClient;
use crate::store::sqlite::{PersistedTokenSet, Profile, Store, unix_now};

#[derive(Debug, Clone)]
pub struct AuthLoginArgs {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub scopes: String,
    pub auth_issuer: String,
}

#[derive(Debug, Serialize)]
pub struct AuthLoginOutput {
    pub ok: bool,
    pub profile: String,
    pub base_url: String,
    pub expires_at: Option<i64>,
    pub scope: Option<String>,
}

impl Default for AuthLoginArgs {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            client_secret: String::new(),
            redirect_uri: DEFAULT_REDIRECT_URI.to_owned(),
            scopes: DEFAULT_SCOPES.to_owned(),
            auth_issuer: DEFAULT_AUTH_ISSUER.to_owned(),
        }
    }
}

pub async fn execute(
    store: &Store,
    profile: &Profile,
    base_url: &str,
    args: AuthLoginArgs,
) -> Result<AuthLoginOutput> {
    let client_id = non_empty(args.client_id, "client_id")?;
    let client_secret = non_empty(args.client_secret, "client_secret")?;
    let redirect_uri = non_empty(args.redirect_uri, "redirect_uri")?;
    let scopes = non_empty(args.scopes, "scopes")?;
    let auth_issuer = non_empty(args.auth_issuer, "auth_issuer")?;

    let verifier = random_urlsafe(48);
    let challenge = pkce_challenge(&verifier);
    let state = random_urlsafe(24);
    let http_client = Client::new();
    let discovery = VolvoClient::fetch_discovery(&auth_issuer, &http_client).await?;
    let auth_url = build_authorization_url(
        &discovery.authorization_endpoint,
        &client_id,
        &redirect_uri,
        &scopes,
        &state,
        &challenge,
    )?;

    webbrowser::open(auth_url.as_str()).with_context(|| {
        format!(
            "failed to open browser for OAuth URL: {}",
            auth_url.as_str()
        )
    })?;
    eprintln!("Opened browser for Volvo login. Waiting for callback on {redirect_uri} ...");

    let code = wait_for_authorization_code(&redirect_uri, &state).await?;
    let token = VolvoClient::exchange_authorization_code(
        &http_client,
        &discovery.token_endpoint,
        &client_id,
        &client_secret,
        &redirect_uri,
        &code,
        &verifier,
    )
    .await?;
    let expires_at = token.expires_in.map(|expires| unix_now() + expires as i64);

    store.save_auth_session(
        profile.id,
        &PersistedTokenSet {
            access_token: token.access_token,
            refresh_token: token.refresh_token,
            scope: token.scope.clone(),
            token_type: token.token_type,
            expires_at,
            token_endpoint: discovery.token_endpoint,
            client_id,
            client_secret,
        },
    )?;

    Ok(AuthLoginOutput {
        ok: true,
        profile: profile.name.clone(),
        base_url: base_url.to_owned(),
        expires_at,
        scope: token.scope,
    })
}

fn non_empty(value: String, field: &str) -> Result<String> {
    let normalized = value.trim().to_owned();
    if normalized.is_empty() {
        return Err(anyhow!("{field} cannot be empty"));
    }
    Ok(normalized)
}

fn random_urlsafe(bytes_len: usize) -> String {
    let mut bytes = vec![0u8; bytes_len];
    rand::rng().fill(bytes.as_mut_slice());
    URL_SAFE_NO_PAD.encode(bytes)
}

fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn build_authorization_url(
    authorization_endpoint: &str,
    client_id: &str,
    redirect_uri: &str,
    scopes: &str,
    state: &str,
    code_challenge: &str,
) -> Result<Url> {
    let mut url =
        Url::parse(authorization_endpoint).context("authorization endpoint URL is invalid")?;
    let query = Serializer::new(String::new())
        .append_pair("response_type", "code")
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", scopes)
        .append_pair("state", state)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256")
        .finish();
    url.set_query(Some(&query));
    Ok(url)
}

async fn wait_for_authorization_code(redirect_uri: &str, expected_state: &str) -> Result<String> {
    let url = Url::parse(redirect_uri).context("redirect URI is invalid")?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("redirect URI must include host"))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| anyhow!("redirect URI must include port"))?;
    let expected_path = url.path().to_owned();

    let listener = TcpListener::bind((host, port))
        .await
        .with_context(|| format!("failed to bind callback server on {host}:{port}"))?;
    let (mut socket, _) = timeout(Duration::from_secs(180), listener.accept())
        .await
        .context("timed out waiting for OAuth callback")?
        .context("failed to accept OAuth callback connection")?;

    let mut buffer = [0_u8; 4096];
    let bytes_read = timeout(Duration::from_secs(10), socket.read(&mut buffer))
        .await
        .context("timed out reading OAuth callback request")?
        .context("failed to read callback request")?;
    if bytes_read == 0 {
        return Err(anyhow!("OAuth callback request was empty"));
    }
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let mut lines = request.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| anyhow!("callback request line missing"))?;
    let path_with_query = parse_request_target(request_line)?;
    let callback_url = Url::parse(&format!("http://localhost{path_with_query}"))
        .context("invalid callback URL")?;
    if callback_url.path() != expected_path {
        return Err(anyhow!(
            "unexpected callback path `{}`; expected `{expected_path}`",
            callback_url.path()
        ));
    }
    let mut code: Option<String> = None;
    let mut state: Option<String> = None;
    for (key, value) in callback_url.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            _ => {}
        }
    }
    let returned_state = state.ok_or_else(|| anyhow!("callback did not include state"))?;
    if returned_state != expected_state {
        return Err(anyhow!("OAuth state mismatch; aborting login"));
    }
    let code = code.ok_or_else(|| anyhow!("callback did not include authorization code"))?;

    socket
        .write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<html><body><h1>Login successful</h1><p>You can return to the terminal.</p></body></html>",
        )
        .await
        .context("failed to write callback response")?;
    Ok(code)
}

fn parse_request_target(request_line: &str) -> Result<&str> {
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| anyhow!("malformed callback request line"))?;
    if method != "GET" {
        return Err(anyhow!("callback method `{method}` is unsupported"));
    }
    parts
        .next()
        .ok_or_else(|| anyhow!("callback request target missing"))
}

#[cfg(test)]
mod tests {
    use super::{build_authorization_url, pkce_challenge};

    #[test]
    fn authorization_url_contains_pkce_and_state() {
        let url = build_authorization_url(
            "https://example.com/auth",
            "client-a",
            "http://127.0.0.1:8787/callback",
            "openid",
            "state-1",
            "challenge-1",
        )
        .expect("URL should build");
        let query = url.query().expect("query should exist");
        assert!(query.contains("code_challenge=challenge-1"));
        assert!(query.contains("state=state-1"));
    }

    #[test]
    fn pkce_challenge_is_non_empty() {
        assert!(!pkce_challenge("abcdefg").is_empty());
    }
}
