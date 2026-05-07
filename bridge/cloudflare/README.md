# VC Cli Auth Service (Cloudflare)

This package hosts:

- `GET /` homepage
- `GET /privacy.html` privacy notice
- `GET /terms.html` terms and conditions
- `POST /v1/oauth/start` OAuth login bootstrap
- `GET /oauth/callback` Volvo redirect callback + localhost handoff page
- `POST /v1/oauth/refresh` bridge-managed refresh exchange

## Security model

- `VOLVO_CLIENT_ID` and `VOLVO_CLIENT_SECRET` are configured as Worker secrets.
- The public OAuth endpoints always use the Worker's configured Volvo issuer (`VOLVO_AUTH_ISSUER`, or the built-in Volvo default); callers cannot override issuer discovery.
- Discovered authorization and token endpoints must use HTTPS and share the configured issuer origin before the Worker sends confidential client credentials.
- Access and refresh tokens are **not persisted** in bridge storage.
- KV stores only short-lived pre-auth session metadata (`state`, PKCE verifier, loopback callback URL, nonce).
- Callback handoff target is limited to loopback callback URLs (`127.0.0.1`, `localhost`, `::1`).

## Setup

```bash
cd bridge/cloudflare
npm install
cp wrangler.example.jsonc wrangler.jsonc
```

Create KV namespace and update local `wrangler.jsonc` `kv_namespaces[0].id`.

```bash
npx wrangler kv namespace create OAUTH_SESSIONS
```

`wrangler.jsonc` is gitignored for open source safety. Commit changes to
`wrangler.example.jsonc` instead when defaults need updates.

Set Worker secrets:

```bash
npx wrangler secret put VOLVO_CLIENT_ID
npx wrangler secret put VOLVO_CLIENT_SECRET
```

Optional issuer override (default already set in `wrangler.jsonc`):

```bash
npx wrangler secret put VOLVO_AUTH_ISSUER
```

## Run and deploy

```bash
npm run dev
npm run deploy
```

Use the deployed URL in CLI:

```bash
export VOLVO_AUTH_BRIDGE_URL="https://<your-domain>"
```

## Rate limiting (aggressive defaults)

This Worker enforces rate limits on **every request** before route handling:

- Per-IP limiter (`RATE_LIMIT_PER_IP`): `8` requests / `10` seconds
- Global limiter (`RATE_LIMIT_GLOBAL`): `120` requests / `10` seconds
- Refresh limiter (`RATE_LIMIT_REFRESH_PER_IP`): `1` request / `60` seconds per IP on `POST /v1/oauth/refresh`
- OAuth start limiter (`RATE_LIMIT_START_PER_IP`): `1` request / `60` seconds per IP on `POST /v1/oauth/start`
- OAuth start global limiter (`RATE_LIMIT_START_GLOBAL`): `10` requests / `10` seconds on `POST /v1/oauth/start`

Both limiters are configured in `wrangler.jsonc` (and `wrangler.example.jsonc`).

### Why two layers

- Per-IP protects against noisy clients.
- Global protects total Worker spend during broad traffic spikes.

### Add WAF rate limiting as a second guardrail

Workers rate limiting counters are local to Cloudflare locations, so add WAF rules for account-level bill protection:

1. Create a WAF rate limiting rule for `/v1/oauth/start` and `/v1/oauth/refresh`.
2. Track by client IP.
3. Use a strict threshold (for example, 10-20 requests/minute per IP).
4. Action: `Managed Challenge` or `Block` with a mitigation timeout.

Using both Worker bindings + WAF gives better protection against unexpected traffic bills.
