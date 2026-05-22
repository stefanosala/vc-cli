use anyhow::{Context, Result, anyhow};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::Rng;
use reqwest::Client;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{self, Write};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{Duration, timeout};
use url::Url;

use crate::config::{
    DEFAULT_AUTH_ISSUER, DEFAULT_AUTH_LISTEN_TIMEOUT_SECONDS, DEFAULT_AUTH_REDIRECT_URI,
    DEFAULT_SCOPES, save_config_values,
};
use crate::http::client::{OAuthDiscovery, OAuthTokenResponse, VolvoClient};
use crate::http::normalize_base_url;
use crate::store::sqlite::{PersistedTokenSet, Profile, Store, unix_now};

#[derive(Debug, Clone)]
pub struct AuthLoginArgs {
    pub scopes: String,
    pub auth_issuer: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub redirect_uri: Option<String>,
    pub auth_listen_timeout_seconds: u64,
    pub headless: bool,
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
            auth_issuer: Some(DEFAULT_AUTH_ISSUER.to_owned()),
            client_id: None,
            client_secret: None,
            redirect_uri: Some(DEFAULT_AUTH_REDIRECT_URI.to_owned()),
            auth_listen_timeout_seconds: DEFAULT_AUTH_LISTEN_TIMEOUT_SECONDS,
            headless: false,
        }
    }
}

pub async fn execute(
    store: &Store,
    profile: &Profile,
    base_url: &str,
    config_dir: &Path,
    args: AuthLoginArgs,
) -> Result<AuthLoginOutput> {
    let scopes = non_empty(args.scopes, "scopes")?;
    let auth_issuer = non_empty_option(args.auth_issuer, "auth issuer", "VOLVO_AUTH_ISSUER")?;
    let redirect_uri = non_empty_option(args.redirect_uri, "redirect URI", "VOLVO_REDIRECT_URI")?;
    if credential_setup_instructions_needed(
        args.client_id.as_deref(),
        args.client_secret.as_deref(),
    ) {
        print_credential_setup_instructions(&redirect_uri);
    }
    let (api_key, save_api_key) =
        resolve_or_prompt_config_value("VCC_API_KEY", "VCC API key", None, true)?;
    let (client_id, save_client_id) = resolve_or_prompt_config_value(
        "VOLVO_CLIENT_ID",
        "Volvo OAuth client ID",
        args.client_id,
        false,
    )?;
    let (client_secret, save_client_secret) = resolve_or_prompt_config_value(
        "VOLVO_CLIENT_SECRET",
        "Volvo OAuth client secret",
        args.client_secret,
        true,
    )?;
    let values_to_save = [
        save_api_key.then_some(("VCC_API_KEY", api_key.as_str())),
        save_client_id.then_some(("VOLVO_CLIENT_ID", client_id.as_str())),
        save_client_secret.then_some(("VOLVO_CLIENT_SECRET", client_secret.as_str())),
    ];
    let values_to_save = values_to_save.into_iter().flatten().collect::<Vec<_>>();
    save_config_values(config_dir, &values_to_save)?;
    let listen_timeout_seconds = normalize_timeout(args.auth_listen_timeout_seconds)?;
    let http_client = Client::new();
    let token_set = local_oauth_login(
        &http_client,
        &auth_issuer,
        &client_id,
        &client_secret,
        &redirect_uri,
        &scopes,
        listen_timeout_seconds,
        args.headless,
    )
    .await?;

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

fn non_empty_option(value: Option<String>, field: &str, env_name: &str) -> Result<String> {
    let provided = value.ok_or_else(|| anyhow!("auth login requires {field}; set {env_name}"))?;
    non_empty(provided, field)
}

fn credential_setup_instructions_needed(
    client_id: Option<&str>,
    client_secret: Option<&str>,
) -> bool {
    missing_env_value("VCC_API_KEY")
        || provided_or_env_missing(client_id, "VOLVO_CLIENT_ID")
        || provided_or_env_missing(client_secret, "VOLVO_CLIENT_SECRET")
}

fn provided_or_env_missing(provided: Option<&str>, env_name: &str) -> bool {
    provided.map(str::trim).unwrap_or_default().is_empty() && missing_env_value(env_name)
}

fn missing_env_value(env_name: &str) -> bool {
    std::env::var(env_name)
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
}

fn print_credential_setup_instructions(redirect_uri: &str) {
    eprintln!(
        "\
Volvo developer credentials are required before login can continue.

1. Create and publish a new app at:
   https://developer.volvocars.com/account/
2. Configure this OAuth redirect URL for the app:
   {redirect_uri}
3. After the app is published, return to this terminal and enter the API key, client ID, and client secret.
"
    );
}

fn resolve_or_prompt_config_value(
    env_name: &str,
    label: &str,
    provided: Option<String>,
    hidden: bool,
) -> Result<(String, bool)> {
    if let Some(value) = provided {
        return Ok((non_empty(value, label)?, true));
    }

    if let Ok(value) = std::env::var(env_name) {
        let normalized = value.trim().to_owned();
        if !normalized.is_empty() {
            return Ok((normalized, false));
        }
    }

    let value = if hidden {
        rpassword::prompt_password(format!("{label}: "))
            .with_context(|| format!("failed to read {label}"))?
    } else {
        prompt_text(label)?
    };
    Ok((non_empty(value, label)?, true))
}

fn prompt_text(label: &str) -> Result<String> {
    let mut stderr = io::stderr();
    write!(stderr, "{label}: ")?;
    stderr.flush()?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .with_context(|| format!("failed to read {label}"))?;
    Ok(input)
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

#[derive(Debug)]
struct AuthorizationCallback {
    code: String,
}

async fn local_oauth_login(
    http_client: &Client,
    auth_issuer: &str,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    scopes: &str,
    listen_timeout_seconds: u64,
    headless: bool,
) -> Result<PersistableTokenSet> {
    let normalized_issuer = normalize_base_url(auth_issuer)?;
    let issuer_url = validate_auth_issuer(&normalized_issuer)?;
    let discovery = VolvoClient::fetch_discovery(&normalized_issuer, http_client).await?;
    validate_oauth_discovery(&issuer_url, &discovery)?;
    let state = random_urlsafe(24);
    let code_verifier = random_urlsafe(64);
    let code_challenge = pkce_challenge(&code_verifier);
    let authorization_url = build_authorization_url(
        &discovery.authorization_endpoint,
        client_id,
        redirect_uri,
        scopes,
        &state,
        &code_challenge,
    )?;
    let callback = if headless {
        let expected_path = expected_redirect_path(redirect_uri)?;
        prompt_headless_callback(&authorization_url, &expected_path, &state)?
    } else {
        let (listener, expected_path) = bind_redirect_listener(redirect_uri).await?;
        webbrowser::open(authorization_url.as_str()).with_context(|| {
            format!("failed to open browser for Volvo OAuth URL: {authorization_url}")
        })?;
        eprintln!(
            "Opened browser for Volvo login. Waiting for OAuth callback on {redirect_uri} ..."
        );
        wait_for_authorization_callback(listener, &expected_path, &state, listen_timeout_seconds)
            .await?
    };
    let token_response = VolvoClient::exchange_authorization_code(
        http_client,
        &discovery.token_endpoint,
        client_id,
        client_secret,
        redirect_uri,
        &callback.code,
        &code_verifier,
    )
    .await?;

    persistable_token_set(
        token_response,
        discovery.token_endpoint,
        client_id,
        client_secret,
    )
}

fn prompt_headless_callback(
    authorization_url: &Url,
    expected_path: &str,
    expected_state: &str,
) -> Result<AuthorizationCallback> {
    eprintln!(
        "\
Headless login mode:
1. Open this URL in any browser where you can sign in:
   {authorization_url}
2. Complete login/consent.
3. Copy the final redirected URL from the browser and paste it below.
"
    );
    let pasted = prompt_text("Paste redirected callback URL")?;
    parse_pasted_callback(&pasted, expected_path, expected_state)
}

fn parse_pasted_callback(
    pasted: &str,
    expected_path: &str,
    expected_state: &str,
) -> Result<AuthorizationCallback> {
    let raw = pasted.trim();
    if raw.is_empty() {
        return Err(anyhow!("callback URL cannot be empty"));
    }

    let callback_url = if raw.starts_with("http://") || raw.starts_with("https://") {
        Url::parse(raw).context("invalid callback URL")?
    } else if raw.starts_with('/') {
        Url::parse(&format!("http://localhost{raw}")).context("invalid callback path")?
    } else if raw.contains('=') {
        Url::parse(&format!(
            "http://localhost{expected_path}?{}",
            raw.trim_start_matches('?')
        ))
        .context("invalid callback query")?
    } else {
        return Err(anyhow!(
            "invalid callback input; paste a full URL (or path/query containing code and state)"
        ));
    };

    if callback_url.path() != expected_path {
        return Err(anyhow!(
            "unexpected callback path `{}`; expected `{expected_path}`",
            callback_url.path()
        ));
    }
    let params = callback_url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect::<Vec<(String, String)>>();
    let get = |key: &str| -> Option<String> {
        params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    };
    if let Some(error) = get("error") {
        let detail = get("error_description").unwrap_or_default();
        return Err(anyhow!("OAuth callback failed: {error} {detail}"));
    }
    let state = get("state").ok_or_else(|| anyhow!("OAuth callback did not include state"))?;
    if state != expected_state {
        return Err(anyhow!("OAuth callback state mismatch"));
    }
    let code = get("code").ok_or_else(|| anyhow!("OAuth callback did not include code"))?;
    Ok(AuthorizationCallback { code })
}

fn validate_auth_issuer(auth_issuer: &str) -> Result<Url> {
    let issuer_url = Url::parse(auth_issuer).context("auth issuer is invalid")?;
    if issuer_url.scheme() != "https" {
        return Err(anyhow!("auth issuer must use https"));
    }
    Ok(issuer_url)
}

fn validate_oauth_discovery(issuer_url: &Url, discovery: &OAuthDiscovery) -> Result<()> {
    validate_discovered_endpoint(
        issuer_url,
        &discovery.authorization_endpoint,
        "authorization endpoint",
    )?;
    validate_discovered_endpoint(issuer_url, &discovery.token_endpoint, "token endpoint")
}

fn validate_discovered_endpoint(issuer_url: &Url, endpoint: &str, label: &str) -> Result<()> {
    let endpoint_url = Url::parse(endpoint).with_context(|| format!("{label} is invalid"))?;
    if endpoint_url.scheme() != "https" {
        return Err(anyhow!("{label} must use https"));
    }
    if !same_origin(issuer_url, &endpoint_url) {
        return Err(anyhow!("{label} must share the auth issuer origin"));
    }

    let issuer_path = issuer_url.path().trim_end_matches('/');
    if !issuer_path.is_empty()
        && issuer_path != "/"
        && !endpoint_url.path().starts_with(&format!("{issuer_path}/"))
        && endpoint_url.path() != issuer_path
    {
        return Err(anyhow!("{label} must be within the auth issuer path"));
    }
    Ok(())
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
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
        Url::parse(authorization_endpoint).context("authorization endpoint is invalid")?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", scopes)
        .append_pair("state", state)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(url)
}

fn persistable_token_set(
    token_response: OAuthTokenResponse,
    token_endpoint: String,
    client_id: &str,
    client_secret: &str,
) -> Result<PersistableTokenSet> {
    Ok(PersistableTokenSet {
        access_token: token_response.access_token,
        refresh_token: token_response.refresh_token,
        scope: token_response.scope,
        token_type: token_response.token_type,
        expires_in: token_response.expires_in,
        token_endpoint,
        client_id: client_id.to_owned(),
        client_secret: client_secret.to_owned(),
    })
}

enum CallbackListener {
    Single(TcpListener),
    Dual {
        ipv4: TcpListener,
        ipv6: TcpListener,
    },
}

async fn bind_redirect_listener(redirect_uri: &str) -> Result<(CallbackListener, String)> {
    let redirect_url = Url::parse(redirect_uri).context("redirect URI is invalid")?;
    if redirect_url.scheme() != "http" {
        return Err(anyhow!(
            "redirect URI must use http for local OAuth callbacks"
        ));
    }
    let host = redirect_url
        .host_str()
        .ok_or_else(|| anyhow!("redirect URI must include a host"))?;
    let port = redirect_url
        .port()
        .ok_or_else(|| anyhow!("redirect URI must include an explicit port"))?;
    if !is_loopback_redirect_host(host) {
        return Err(anyhow!(
            "redirect URI host `{host}` is not a supported loopback host"
        ));
    }

    let listener = if host.eq_ignore_ascii_case("localhost") {
        bind_localhost_listeners(port).await?
    } else if host == "::1" {
        CallbackListener::Single(
            TcpListener::bind((Ipv6Addr::LOCALHOST, port))
                .await
                .with_context(|| {
                    format!("failed to bind OAuth callback listener on {redirect_uri}")
                })?,
        )
    } else {
        CallbackListener::Single(
            TcpListener::bind((Ipv4Addr::LOCALHOST, port))
                .await
                .with_context(|| {
                    format!("failed to bind OAuth callback listener on {redirect_uri}")
                })?,
        )
    };

    let expected_path = if redirect_url.path().is_empty() {
        "/".to_owned()
    } else {
        redirect_url.path().to_owned()
    };
    Ok((listener, expected_path))
}

fn expected_redirect_path(redirect_uri: &str) -> Result<String> {
    let redirect_url = Url::parse(redirect_uri).context("redirect URI is invalid")?;
    if redirect_url.scheme() != "http" {
        return Err(anyhow!(
            "redirect URI must use http for local OAuth callbacks"
        ));
    }
    let host = redirect_url
        .host_str()
        .ok_or_else(|| anyhow!("redirect URI must include a host"))?;
    if !is_loopback_redirect_host(host) {
        return Err(anyhow!(
            "redirect URI host `{host}` is not a supported loopback host"
        ));
    }
    redirect_url
        .port()
        .ok_or_else(|| anyhow!("redirect URI must include an explicit port"))?;
    Ok(if redirect_url.path().is_empty() {
        "/".to_owned()
    } else {
        redirect_url.path().to_owned()
    })
}

async fn bind_localhost_listeners(port: u16) -> Result<CallbackListener> {
    let ipv4 = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .await
        .context("failed to bind IPv4 localhost OAuth callback listener")?;
    match TcpListener::bind((Ipv6Addr::LOCALHOST, port)).await {
        Ok(ipv6) => Ok(CallbackListener::Dual { ipv4, ipv6 }),
        Err(_) => Ok(CallbackListener::Single(ipv4)),
    }
}

fn is_loopback_redirect_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host == "127.0.0.1"
        || host == "::1"
        || host.eq_ignore_ascii_case("localtest.me")
        || host.to_ascii_lowercase().ends_with(".localtest.me")
}

async fn wait_for_authorization_callback(
    listener: CallbackListener,
    expected_path: &str,
    expected_state: &str,
    listen_timeout_seconds: u64,
) -> Result<AuthorizationCallback> {
    let (mut socket, _) = timeout(
        Duration::from_secs(listen_timeout_seconds),
        accept_callback(listener),
    )
    .await
    .context("timed out waiting for OAuth callback")?
    .context("failed to accept OAuth callback connection")?;

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
        write_callback_response(
            &mut socket,
            "Login failed",
            "The OAuth callback path did not match the configured redirect URI. You can return to the terminal.",
        )
        .await?;
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
        write_callback_response(
            &mut socket,
            "Login failed",
            "Volvo returned an OAuth error. You can return to the terminal.",
        )
        .await?;
        return Err(anyhow!("OAuth callback failed: {error} {detail}"));
    }
    let state = get("state").ok_or_else(|| anyhow!("OAuth callback did not include state"))?;
    if state != expected_state {
        write_callback_response(
            &mut socket,
            "Login failed",
            "The OAuth callback state did not match this login attempt. You can return to the terminal.",
        )
        .await?;
        return Err(anyhow!("OAuth callback state mismatch"));
    }
    let Some(code) = get("code") else {
        write_callback_response(
            &mut socket,
            "Login failed",
            "The OAuth callback did not include an authorization code. You can return to the terminal.",
        )
        .await?;
        return Err(anyhow!("OAuth callback did not include code"));
    };
    write_callback_response(
        &mut socket,
        "Login successful",
        "You can return to the terminal.",
    )
    .await?;
    Ok(AuthorizationCallback { code })
}

async fn accept_callback(
    listener: CallbackListener,
) -> std::io::Result<(tokio::net::TcpStream, std::net::SocketAddr)> {
    match listener {
        CallbackListener::Single(listener) => listener.accept().await,
        CallbackListener::Dual { ipv4, ipv6 } => {
            tokio::select! {
                result = ipv4.accept() => result,
                result = ipv6.accept() => result,
            }
        }
    }
}

async fn write_callback_response(
    socket: &mut tokio::net::TcpStream,
    title: &str,
    message: &str,
) -> Result<()> {
    let body = format!("<html><body><h1>{title}</h1><p>{message}</p></body></html>");
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    socket
        .write_all(response.as_bytes())
        .await
        .context("failed to write OAuth callback response")
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

#[cfg(test)]
mod tests {
    use super::{
        build_authorization_url, expected_redirect_path, is_loopback_redirect_host,
        parse_form_body, parse_pasted_callback, parse_request_line, pkce_challenge,
        validate_auth_issuer, validate_oauth_discovery,
    };
    use crate::http::client::OAuthDiscovery;

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
    fn authorization_url_includes_pkce_parameters() {
        let full = build_authorization_url(
            "https://example.com/oauth2/auth",
            "client-a",
            "http://127.0.0.1:1410/callback",
            "openid conve:vehicle_relation",
            "state-a",
            "challenge-a",
        )
        .expect("authorization URL should build");
        let query = full.query().expect("query should exist");
        assert!(query.contains("response_type=code"));
        assert!(query.contains("client_id=client-a"));
        assert!(query.contains("code_challenge=challenge-a"));
        assert!(query.contains("code_challenge_method=S256"));
    }

    #[test]
    fn pkce_challenge_uses_s256() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn loopback_redirect_hosts_include_localtest() {
        assert!(is_loopback_redirect_host("127.0.0.1"));
        assert!(is_loopback_redirect_host("localhost"));
        assert!(is_loopback_redirect_host("vc-cli.localtest.me"));
        assert!(!is_loopback_redirect_host("example.com"));
    }

    #[test]
    fn auth_issuer_must_use_https() {
        assert!(validate_auth_issuer("https://volvoid.eu.volvocars.com").is_ok());
        assert!(validate_auth_issuer("http://volvoid.eu.volvocars.com").is_err());
    }

    #[test]
    fn discovery_endpoints_must_match_issuer_origin() {
        let issuer =
            validate_auth_issuer("https://volvoid.eu.volvocars.com").expect("valid issuer");
        let valid = OAuthDiscovery {
            authorization_endpoint: "https://volvoid.eu.volvocars.com/oauth2/auth".to_owned(),
            token_endpoint: "https://volvoid.eu.volvocars.com/oauth2/token".to_owned(),
        };
        assert!(validate_oauth_discovery(&issuer, &valid).is_ok());

        let cross_origin = OAuthDiscovery {
            authorization_endpoint: "https://evil.example.com/oauth2/auth".to_owned(),
            token_endpoint: "https://volvoid.eu.volvocars.com/oauth2/token".to_owned(),
        };
        assert!(validate_oauth_discovery(&issuer, &cross_origin).is_err());

        let insecure = OAuthDiscovery {
            authorization_endpoint: "https://volvoid.eu.volvocars.com/oauth2/auth".to_owned(),
            token_endpoint: "http://volvoid.eu.volvocars.com/oauth2/token".to_owned(),
        };
        assert!(validate_oauth_discovery(&issuer, &insecure).is_err());
    }

    #[test]
    fn expected_redirect_path_validates_loopback_http_uri() {
        let path =
            expected_redirect_path("http://127.0.0.1:1410/callback").expect("path should parse");
        assert_eq!(path, "/callback");
        assert!(expected_redirect_path("https://127.0.0.1:1410/callback").is_err());
        assert!(expected_redirect_path("http://example.com:1410/callback").is_err());
    }

    #[test]
    fn parse_pasted_callback_accepts_full_url() {
        let callback = parse_pasted_callback(
            "http://127.0.0.1:1410/callback?code=abc&state=xyz",
            "/callback",
            "xyz",
        )
        .expect("callback should parse");
        assert_eq!(callback.code, "abc");
    }

    #[test]
    fn parse_pasted_callback_accepts_query_only_input() {
        let callback =
            parse_pasted_callback("code=abc&state=xyz", "/callback", "xyz").expect("should parse");
        assert_eq!(callback.code, "abc");
    }

    #[test]
    fn parse_pasted_callback_rejects_state_mismatch() {
        assert!(
            parse_pasted_callback(
                "http://127.0.0.1:1410/callback?code=abc&state=wrong",
                "/callback",
                "xyz"
            )
            .is_err()
        );
    }
}
