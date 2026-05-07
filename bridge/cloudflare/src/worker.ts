interface Env {
  ASSETS: Fetcher;
  OAUTH_SESSIONS: KVNamespace;
  VOLVO_CLIENT_ID: string;
  VOLVO_CLIENT_SECRET: string;
  VOLVO_AUTH_ISSUER?: string;
}

interface BridgeStartRequest {
  scope?: string;
  auth_issuer?: string;
  local_callback_url: string;
  nonce: string;
}

interface BridgeSession {
  state: string;
  codeVerifier: string;
  localCallbackUrl: string;
  nonce: string;
  tokenEndpoint: string;
  createdAt: number;
}

interface OidcDiscovery {
  authorization_endpoint: string;
  token_endpoint: string;
}

const SESSION_TTL_SECONDS = 10 * 60;
const DEFAULT_SCOPE = "openid";
const DEFAULT_AUTH_ISSUER = "https://volvoid.eu.volvocars.com";

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const path = url.pathname;

    if (request.method === "POST" && path === "/v1/oauth/start") {
      return handleStart(request, env, url.origin);
    }
    if (request.method === "GET" && path === "/oauth/callback") {
      return handleCallback(request, env, url.origin);
    }
    if (request.method === "POST" && path === "/v1/oauth/refresh") {
      return handleRefresh(request, env);
    }

    if (env.ASSETS) {
      return env.ASSETS.fetch(request);
    }
    return new Response("Not found", { status: 404 });
  },
};

async function handleStart(request: Request, env: Env, origin: string): Promise<Response> {
  const body = await readJson<BridgeStartRequest>(request);
  const localCallbackUrl = normalizeLocalCallbackUrl(body.local_callback_url);
  const nonce = normalizeRequired(body.nonce, "nonce");
  const scope = normalizeOptional(body.scope) ?? DEFAULT_SCOPE;
  const authIssuer = normalizeOptional(body.auth_issuer) ?? env.VOLVO_AUTH_ISSUER ?? DEFAULT_AUTH_ISSUER;
  const clientId = normalizeRequired(env.VOLVO_CLIENT_ID, "VOLVO_CLIENT_ID");

  const discovery = await fetchDiscovery(authIssuer);
  const sessionId = randomUrlSafe(24);
  const stateSuffix = randomUrlSafe(24);
  const state = `${sessionId}.${stateSuffix}`;
  const codeVerifier = randomUrlSafe(48);
  const codeChallenge = await sha256Base64Url(codeVerifier);
  const redirectUri = `${origin}/oauth/callback`;

  const session: BridgeSession = {
    state,
    codeVerifier,
    localCallbackUrl,
    nonce,
    tokenEndpoint: discovery.token_endpoint,
    createdAt: Date.now(),
  };
  await env.OAUTH_SESSIONS.put(sessionKey(sessionId), JSON.stringify(session), {
    expirationTtl: SESSION_TTL_SECONDS,
  });

  const authorizationUrl = new URL(discovery.authorization_endpoint);
  authorizationUrl.searchParams.set("response_type", "code");
  authorizationUrl.searchParams.set("client_id", clientId);
  authorizationUrl.searchParams.set("redirect_uri", redirectUri);
  authorizationUrl.searchParams.set("scope", scope);
  authorizationUrl.searchParams.set("state", state);
  authorizationUrl.searchParams.set("code_challenge", codeChallenge);
  authorizationUrl.searchParams.set("code_challenge_method", "S256");

  return json({
    session_id: sessionId,
    authorization_url: authorizationUrl.toString(),
    expires_in_seconds: SESSION_TTL_SECONDS,
  });
}

async function handleCallback(request: Request, env: Env, origin: string): Promise<Response> {
  const url = new URL(request.url);
  const error = url.searchParams.get("error");
  if (error) {
    const description = url.searchParams.get("error_description") ?? "";
    return html(`<h1>Login failed</h1><p>${escapeHtml(error)} ${escapeHtml(description)}</p>`, 400);
  }

  const code = normalizeRequired(url.searchParams.get("code"), "code");
  const state = normalizeRequired(url.searchParams.get("state"), "state");
  const sessionId = parseSessionId(state);
  const sessionRaw = await env.OAUTH_SESSIONS.get(sessionKey(sessionId));
  if (!sessionRaw) {
    return html("<h1>Session expired</h1><p>Please run login again.</p>", 400);
  }
  const session = parseSession(sessionRaw);
  if (session.state !== state) {
    return html("<h1>State mismatch</h1><p>Please run login again.</p>", 400);
  }

  await env.OAUTH_SESSIONS.delete(sessionKey(sessionId));

  try {
    const tokenResponse = await exchangeAuthorizationCode(env, session, code, `${origin}/oauth/callback`);
    return handoffPage(session.localCallbackUrl, {
      nonce: session.nonce,
      access_token: tokenResponse.access_token,
      refresh_token: tokenResponse.refresh_token ?? "",
      token_type: tokenResponse.token_type ?? "",
      scope: tokenResponse.scope ?? "",
      expires_in: tokenResponse.expires_in ? String(tokenResponse.expires_in) : "",
    });
  } catch (err) {
    const message = err instanceof Error ? err.message : "unknown callback error";
    return handoffPage(session.localCallbackUrl, {
      nonce: session.nonce,
      error: "token_exchange_failed",
      error_description: message,
    });
  }
}

async function handleRefresh(request: Request, env: Env): Promise<Response> {
  const body = await readJson<{ refresh_token?: string; auth_issuer?: string }>(request);
  const refreshToken = normalizeRequired(body.refresh_token, "refresh_token");
  const authIssuer = normalizeOptional(body.auth_issuer) ?? env.VOLVO_AUTH_ISSUER ?? DEFAULT_AUTH_ISSUER;
  const discovery = await fetchDiscovery(authIssuer);
  const response = await fetch(discovery.token_endpoint, {
    method: "POST",
    headers: {
      accept: "application/json",
      "content-type": "application/x-www-form-urlencoded",
    },
    body: new URLSearchParams({
      grant_type: "refresh_token",
      client_id: normalizeRequired(env.VOLVO_CLIENT_ID, "VOLVO_CLIENT_ID"),
      client_secret: normalizeRequired(env.VOLVO_CLIENT_SECRET, "VOLVO_CLIENT_SECRET"),
      refresh_token: refreshToken,
    }),
  });
  const text = await response.text();
  if (!response.ok) {
    return json({ error: "refresh_failed", detail: truncate(text) }, response.status);
  }
  return new Response(text, {
    status: 200,
    headers: {
      "content-type": "application/json",
      "cache-control": "no-store",
    },
  });
}

async function exchangeAuthorizationCode(
  env: Env,
  session: BridgeSession,
  code: string,
  redirectUri: string,
): Promise<{
  access_token: string;
  token_type?: string;
  expires_in?: number;
  refresh_token?: string;
  scope?: string;
}> {
  const response = await fetch(session.tokenEndpoint, {
    method: "POST",
    headers: {
      accept: "application/json",
      "content-type": "application/x-www-form-urlencoded",
    },
    body: new URLSearchParams({
      grant_type: "authorization_code",
      client_id: normalizeRequired(env.VOLVO_CLIENT_ID, "VOLVO_CLIENT_ID"),
      client_secret: normalizeRequired(env.VOLVO_CLIENT_SECRET, "VOLVO_CLIENT_SECRET"),
      redirect_uri: redirectUri,
      code,
      code_verifier: session.codeVerifier,
    }),
  });
  const text = await response.text();
  if (!response.ok) {
    throw new Error(`token_exchange_failed: ${truncate(text)}`);
  }
  const parsed = JSON.parse(text) as {
    access_token?: string;
    token_type?: string;
    expires_in?: number;
    refresh_token?: string;
    scope?: string;
  };
  if (!parsed.access_token) {
    throw new Error("token_exchange_failed: access_token missing");
  }
  return parsed as {
    access_token: string;
    token_type?: string;
    expires_in?: number;
    refresh_token?: string;
    scope?: string;
  };
}

async function fetchDiscovery(issuerRaw: string): Promise<OidcDiscovery> {
  const issuer = normalizeIssuer(issuerRaw);
  const response = await fetch(`${issuer}/.well-known/openid-configuration`, {
    headers: { accept: "application/json" },
  });
  if (!response.ok) {
    throw new Error(`oidc_discovery_failed: ${response.status}`);
  }
  const parsed = (await response.json()) as OidcDiscovery;
  if (!parsed.authorization_endpoint || !parsed.token_endpoint) {
    throw new Error("oidc_discovery_invalid");
  }
  return parsed;
}

function handoffPage(callbackUrl: string, fields: Record<string, string>): Response {
  const inputs = Object.entries(fields)
    .map(
      ([name, value]) =>
        `<input type="hidden" name="${escapeHtml(name)}" value="${escapeHtml(value)}" />`,
    )
    .join("\n");
  const body = `<!doctype html>
<html lang="en">
  <head><meta charset="utf-8" /><title>Completing login</title></head>
  <body>
    <h1>Completing login...</h1>
    <p>If you are not redirected automatically, click continue.</p>
    <form id="handoff" method="POST" action="${escapeHtml(callbackUrl)}">
      ${inputs}
      <button type="submit">Continue</button>
    </form>
    <script>document.getElementById("handoff").submit();</script>
  </body>
</html>`;
  return html(body, 200);
}

function parseSession(raw: string): BridgeSession {
  const parsed = JSON.parse(raw) as Partial<BridgeSession>;
  if (
    !parsed.state ||
    !parsed.codeVerifier ||
    !parsed.localCallbackUrl ||
    !parsed.nonce ||
    !parsed.tokenEndpoint ||
    typeof parsed.createdAt !== "number"
  ) {
    throw new Error("invalid_session_payload");
  }
  return {
    state: parsed.state,
    codeVerifier: parsed.codeVerifier,
    localCallbackUrl: parsed.localCallbackUrl,
    nonce: parsed.nonce,
    tokenEndpoint: parsed.tokenEndpoint,
    createdAt: parsed.createdAt,
  };
}

function parseSessionId(state: string): string {
  const firstDot = state.indexOf(".");
  if (firstDot <= 0) {
    throw new Error("invalid_state");
  }
  return state.slice(0, firstDot);
}

function sessionKey(sessionId: string): string {
  return `oauth:${sessionId}`;
}

function normalizeLocalCallbackUrl(raw: string): string {
  const parsed = new URL(normalizeRequired(raw, "local_callback_url"));
  if (parsed.protocol !== "http:") {
    throw new Error("local_callback_url must use http");
  }
  const host = parsed.hostname.toLowerCase();
  if (host !== "127.0.0.1" && host !== "localhost" && host !== "::1") {
    throw new Error("local_callback_url host must be loopback");
  }
  return parsed.toString();
}

function normalizeIssuer(raw: string): string {
  const parsed = new URL(normalizeRequired(raw, "auth_issuer"));
  parsed.pathname = parsed.pathname.replace(/\/+$/, "");
  parsed.search = "";
  parsed.hash = "";
  return parsed.toString().replace(/\/$/, "");
}

function normalizeRequired(raw: string | null | undefined, field: string): string {
  const value = (raw ?? "").trim();
  if (!value) {
    throw new Error(`${field}_required`);
  }
  return value;
}

function normalizeOptional(raw: string | null | undefined): string | undefined {
  const value = (raw ?? "").trim();
  return value ? value : undefined;
}

function randomUrlSafe(bytesLength: number): string {
  const bytes = new Uint8Array(bytesLength);
  crypto.getRandomValues(bytes);
  return toBase64Url(bytes);
}

async function sha256Base64Url(value: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
  return toBase64Url(new Uint8Array(digest));
}

function toBase64Url(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function truncate(value: string): string {
  return value.length > 300 ? `${value.slice(0, 300)}...` : value;
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

async function readJson<T>(request: Request): Promise<T> {
  const contentType = request.headers.get("content-type")?.toLowerCase() ?? "";
  if (!contentType.includes("application/json")) {
    throw new Error("content_type_must_be_application_json");
  }
  return (await request.json()) as T;
}

function json(payload: unknown, status = 200): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: {
      "content-type": "application/json",
      "cache-control": "no-store",
    },
  });
}

function html(body: string, status = 200): Response {
  return new Response(body, {
    status,
    headers: {
      "content-type": "text/html; charset=utf-8",
      "cache-control": "no-store",
    },
  });
}
