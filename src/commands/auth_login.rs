use anyhow::{Context, Result, anyhow};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::Rng;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{Duration, timeout};
use url::Url;
use url::form_urlencoded::Serializer;

use crate::config::{
    DEFAULT_AUTH_ISSUER, DEFAULT_AUTH_LISTEN_TIMEOUT_SECONDS, DEFAULT_REDIRECT_URI, DEFAULT_SCOPES,
};
use crate::http::client::VolvoClient;
use crate::http::normalize_base_url;
use crate::store::sqlite::{PersistedTokenSet, Profile, Store, unix_now};

const BRIDGE_MANAGED_SENTINEL: &str = "bridge-managed";
const BRIDGE_REFRESH_PATH: &str = "/v1/oauth/refresh";

#[derive(Debug, Clone)]
pub struct AuthLoginArgs {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub redirect_uri: String,
    pub scopes: String,
    pub auth_issuer: String,
    pub auth_bridge_url: Option<String>,
    pub auth_listen_timeout_seconds: u64,
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
            client_id: None,
            client_secret: None,
            redirect_uri: DEFAULT_REDIRECT_URI.to_owned(),
            scopes: DEFAULT_SCOPES.to_owned(),
            auth_issuer: DEFAULT_AUTH_ISSUER.to_owned(),
            auth_bridge_url: None,
            auth_listen_timeout_seconds: DEFAULT_AUTH_LISTEN_TIMEOUT_SECONDS,
        }
    }
}

pub async fn execute(
    store: &Store,
    profile: &Profile,
    base_url: &str,
    args: AuthLoginArgs,
) -> Result<AuthLoginOutput> {
    let scopes = non_empty(args.scopes, "scopes")?;
    let listen_timeout_seconds = normalize_timeout(args.auth_listen_timeout_seconds)?;
    let http_client = Client::new();

    let token_set = if let Some(bridge_url) = normalize_optional(args.auth_bridge_url) {
        bridge_login(&http_client, &bridge_url, &scopes, listen_timeout_seconds).await?
    } else {
        let auth_issuer = non_empty(args.auth_issuer, "auth_issuer")?;
        legacy_login(
            &http_client,
            non_empty_option(args.client_id, "client_id")?,
            non_empty_option(args.client_secret, "client_secret")?,
            non_empty(args.redirect_uri, "redirect_uri")?,
            &auth_issuer,
            &scopes,
            listen_timeout_seconds,
        )
        .await?
    };

    let expires_at = token_set
        .expires_in
        .map(|expires| unix_now() + expires as i64);

    store.save_auth_session(
        profile.id,
        &PersistedTokenSet {
            access_token: token_set.access_token,
            refresh_token: token_set.refresh_token,
            scope: token_set.scope.clone(),
            token_type: token_set.token_type,
            expires_at,
            token_endpoint: token_set.token_endpoint,
            client_id: token_set.client_id,
            client_secret: token_set.client_secret,
        },
    )?;

    Ok(AuthLoginOutput {
        ok: true,
        profile: profile.name.clone(),
        base_url: base_url.to_owned(),
        expires_at,
        scope: token_set.scope,
    })
}

fn non_empty(value: String, field: &str) -> Result<String> {
    let normalized = value.trim().to_owned();
    if normalized.is_empty() {
        return Err(anyhow!("{field} cannot be empty"));
    }
    Ok(normalized)
}

fn non_empty_option(value: Option<String>, field: &str) -> Result<String> {
    let provided = value.ok_or_else(|| anyhow!("{field} is required for local OAuth flow"))?;
    non_empty(provided, field)
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim().to_owned();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn normalize_timeout(value: u64) -> Result<u64> {
    if value == 0 {
        return Err(anyhow!("auth listen timeout must be greater than zero"));
    }
    Ok(value)
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

#[derive(Debug)]
struct PersistableTokenSet {
    access_token: String,
    refresh_token: Option<String>,
    scope: Option<String>,
    token_type: Option<String>,
    expires_in: Option<u64>,
    token_endpoint: String,
    client_id: String,
    client_secret: String,
}

async fn legacy_login(
    http_client: &Client,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    auth_issuer: &str,
    scopes: &str,
    listen_timeout_seconds: u64,
) -> Result<PersistableTokenSet> {
    let verifier = random_urlsafe(48);
    let challenge = pkce_challenge(&verifier);
    let state = random_urlsafe(24);
    let discovery = VolvoClient::fetch_discovery(auth_issuer, http_client).await?;
    let auth_url = build_authorization_url(
        &discovery.authorization_endpoint,
        &client_id,
        &redirect_uri,
        scopes,
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

    let code = wait_for_authorization_code(&redirect_uri, &state, listen_timeout_seconds).await?;
    let token = VolvoClient::exchange_authorization_code(
        http_client,
        &discovery.token_endpoint,
        &client_id,
        &client_secret,
        &redirect_uri,
        &code,
        &verifier,
    )
    .await?;
    Ok(PersistableTokenSet {
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        scope: token.scope,
        token_type: token.token_type,
        expires_in: token.expires_in,
        token_endpoint: discovery.token_endpoint,
        client_id,
        client_secret,
    })
}

#[derive(Debug, Serialize)]
struct BridgeStartRequest {
    scope: String,
    local_callback_url: String,
    nonce: String,
}

#[derive(Debug, Deserialize)]
struct BridgeStartResponse {
    authorization_url: String,
}

#[derive(Debug)]
struct BridgeHandoffPayload {
    access_token: String,
    refresh_token: Option<String>,
    scope: Option<String>,
    token_type: Option<String>,
    expires_in: Option<u64>,
}

async fn bridge_login(
    http_client: &Client,
    bridge_url: &str,
    scopes: &str,
    listen_timeout_seconds: u64,
) -> Result<PersistableTokenSet> {
    let normalized_bridge = normalize_base_url(bridge_url)?;
    let (listener, local_callback_url) = bind_loopback_listener().await?;
    let nonce = random_urlsafe(24);

    let start_response = http_client
        .post(join_url_path(&normalized_bridge, "/v1/oauth/start")?)
        .json(&BridgeStartRequest {
            scope: scopes.to_owned(),
            local_callback_url: local_callback_url.clone(),
            nonce: nonce.clone(),
        })
        .send()
        .await
        .context("failed to start bridge OAuth session")?
        .error_for_status()
        .context("bridge start session request failed")?
        .json::<BridgeStartResponse>()
        .await
        .context("failed to parse bridge start session response")?;

    webbrowser::open(&start_response.authorization_url).with_context(|| {
        format!(
            "failed to open browser for bridge OAuth URL: {}",
            &start_response.authorization_url
        )
    })?;
    eprintln!(
        "Opened browser for Volvo login. Waiting for localhost callback on {local_callback_url} ..."
    );

    let handoff =
        wait_for_bridge_handoff(listener, "/callback", &nonce, listen_timeout_seconds).await?;

    Ok(PersistableTokenSet {
        access_token: handoff.access_token,
        refresh_token: handoff.refresh_token,
        scope: handoff.scope,
        token_type: handoff.token_type,
        expires_in: handoff.expires_in,
        token_endpoint: join_url_path(&normalized_bridge, BRIDGE_REFRESH_PATH)?,
        client_id: BRIDGE_MANAGED_SENTINEL.to_owned(),
        client_secret: BRIDGE_MANAGED_SENTINEL.to_owned(),
    })
}

async fn bind_loopback_listener() -> Result<(TcpListener, String)> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .context("failed to bind localhost callback listener")?;
    let port = listener
        .local_addr()
        .context("failed to resolve localhost callback listener address")?
        .port();
    Ok((listener, format!("http://127.0.0.1:{port}/callback")))
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

async fn wait_for_authorization_code(
    redirect_uri: &str,
    expected_state: &str,
    listen_timeout_seconds: u64,
) -> Result<String> {
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
    let (mut socket, _) = timeout(
        Duration::from_secs(listen_timeout_seconds),
        listener.accept(),
    )
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

async fn wait_for_bridge_handoff(
    listener: TcpListener,
    expected_path: &str,
    expected_nonce: &str,
    listen_timeout_seconds: u64,
) -> Result<BridgeHandoffPayload> {
    let (mut socket, _) = timeout(
        Duration::from_secs(listen_timeout_seconds),
        listener.accept(),
    )
    .await
    .context("timed out waiting for bridge localhost callback")?
    .context("failed to accept bridge localhost callback connection")?;

    let request_bytes = read_http_request(&mut socket).await?;
    let request = String::from_utf8_lossy(&request_bytes);
    let mut lines = request.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| anyhow!("callback request line missing"))?;
    let (method, target) = parse_request_line(request_line)?;
    let callback_url = Url::parse(&format!("http://localhost{target}"))
        .context("invalid localhost callback URL")?;
    if callback_url.path() != expected_path {
        return Err(anyhow!(
            "unexpected callback path `{}`; expected `{expected_path}`",
            callback_url.path()
        ));
    }
    let params = if method == "POST" {
        parse_form_body(&request_bytes)?
    } else {
        callback_url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect::<Vec<(String, String)>>()
    };
    let get = |key: &str| -> Option<String> {
        params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    };
    if let Some(error) = get("error") {
        let detail = get("error_description").unwrap_or_default();
        return Err(anyhow!("bridge callback failed: {error} {detail}"));
    }
    let nonce = get("nonce").ok_or_else(|| anyhow!("bridge callback did not include nonce"))?;
    if nonce != expected_nonce {
        return Err(anyhow!("bridge callback nonce mismatch"));
    }
    let payload = BridgeHandoffPayload {
        access_token: get("access_token")
            .ok_or_else(|| anyhow!("bridge callback did not include access_token"))?,
        refresh_token: get("refresh_token"),
        scope: get("scope"),
        token_type: get("token_type"),
        expires_in: get("expires_in").and_then(|value| value.parse::<u64>().ok()),
    };
    socket
        .write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<html><body><h1>Login successful</h1><p>You can return to the terminal.</p></body></html>",
        )
        .await
        .context("failed to write localhost callback response")?;
    Ok(payload)
}

async fn read_http_request(socket: &mut tokio::net::TcpStream) -> Result<Vec<u8>> {
    let mut buffer = Vec::with_capacity(4096);
    let mut chunk = [0_u8; 1024];
    loop {
        let read = timeout(Duration::from_secs(10), socket.read(&mut chunk))
            .await
            .context("timed out reading callback request")?
            .context("failed to read callback request")?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.len() > 64 * 1024 {
            return Err(anyhow!("callback request too large"));
        }
        if let Some(header_end) = find_header_end(&buffer) {
            let body_len = parse_content_length(&buffer[..header_end]).unwrap_or(0);
            if buffer.len() >= header_end + 4 + body_len {
                break;
            }
        }
    }
    if buffer.is_empty() {
        return Err(anyhow!("OAuth callback request was empty"));
    }
    Ok(buffer)
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_content_length(headers: &[u8]) -> Option<usize> {
    let header_text = String::from_utf8_lossy(headers);
    for line in header_text.lines() {
        let mut parts = line.splitn(2, ':');
        let key = parts.next()?.trim().to_ascii_lowercase();
        if key != "content-length" {
            continue;
        }
        let raw = parts.next()?.trim();
        if let Ok(parsed) = raw.parse::<usize>() {
            return Some(parsed);
        }
    }
    None
}

fn parse_form_body(request_bytes: &[u8]) -> Result<Vec<(String, String)>> {
    let header_end =
        find_header_end(request_bytes).ok_or_else(|| anyhow!("malformed callback request"))?;
    let body = &request_bytes[header_end + 4..];
    let body_str = std::str::from_utf8(body).context("callback body was not UTF-8")?;
    Ok(url::form_urlencoded::parse(body_str.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect())
}

fn parse_request_line(request_line: &str) -> Result<(&str, &str)> {
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| anyhow!("malformed callback request line"))?;
    if method != "GET" && method != "POST" {
        return Err(anyhow!("callback method `{method}` is unsupported"));
    }
    let target = parts
        .next()
        .ok_or_else(|| anyhow!("callback request target missing"))?;
    Ok((method, target))
}

fn parse_request_target(request_line: &str) -> Result<&str> {
    let (method, target) = parse_request_line(request_line)?;
    if method != "GET" {
        return Err(anyhow!("callback method `{method}` is unsupported"));
    }
    Ok(target)
}

fn join_url_path(base: &str, path: &str) -> Result<String> {
    let base_url = Url::parse(base).context("bridge URL is invalid")?;
    Ok(base_url.join(path)?.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        build_authorization_url, join_url_path, parse_form_body, parse_request_line, pkce_challenge,
    };

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

    #[test]
    fn parse_request_line_supports_post() {
        let (method, target) =
            parse_request_line("POST /callback HTTP/1.1").expect("request line should parse");
        assert_eq!(method, "POST");
        assert_eq!(target, "/callback");
    }

    #[test]
    fn parse_form_body_extracts_values() {
        let bytes =
            b"POST /callback HTTP/1.1\r\nHost: localhost\r\nContent-Length: 19\r\n\r\nnonce=a&scope=openid";
        let pairs = parse_form_body(bytes).expect("form body should parse");
        assert!(pairs.iter().any(|(k, v)| k == "nonce" && v == "a"));
        assert!(pairs.iter().any(|(k, v)| k == "scope" && v == "openid"));
    }

    #[test]
    fn join_url_path_appends_path() {
        let full = join_url_path("https://bridge.example.com", "/v1/oauth/refresh")
            .expect("URL should join");
        assert_eq!(full, "https://bridge.example.com/v1/oauth/refresh");
    }
}
