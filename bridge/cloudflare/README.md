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
