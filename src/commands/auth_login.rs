use anyhow::{Context, Result, anyhow};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::Rng;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{Duration, timeout};
use url::Url;

use crate::config::{DEFAULT_AUTH_BRIDGE_URL, DEFAULT_AUTH_LISTEN_TIMEOUT_SECONDS, DEFAULT_SCOPES};
use crate::http::normalize_base_url;
use crate::store::sqlite::{PersistedTokenSet, Profile, Store, unix_now};

const BRIDGE_MANAGED_SENTINEL: &str = "bridge-managed";
const BRIDGE_REFRESH_PATH: &str = "/v1/oauth/refresh";

#[derive(Debug, Clone)]
pub struct AuthLoginArgs {
    pub scopes: String,
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
            scopes: DEFAULT_SCOPES.to_owned(),
            auth_bridge_url: Some(DEFAULT_AUTH_BRIDGE_URL.to_owned()),
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
    let bridge_url = non_empty_option(args.auth_bridge_url, "auth_bridge_url")?;
    let token_set =
        bridge_login(&http_client, &bridge_url, &scopes, listen_timeout_seconds).await?;

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
    let provided = value
        .ok_or_else(|| anyhow!("auth login requires --auth-bridge-url or VOLVO_AUTH_BRIDGE_URL"))?;
    non_empty(provided, field)
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

fn join_url_path(base: &str, path: &str) -> Result<String> {
    let base_url = Url::parse(base).context("bridge URL is invalid")?;
    Ok(base_url.join(path)?.to_string())
}

#[cfg(test)]
mod tests {
    use super::{join_url_path, parse_form_body, parse_request_line};

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
