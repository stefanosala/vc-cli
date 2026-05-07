# VC Cli

VC Cli is a command-line interface for interacting with the Volvo Cars Connected Vehicle API.

## Prerequisites

- Rust toolchain (stable)
- A Volvo developer application with:
  - `VOLVO_CLIENT_ID`
  - `VOLVO_CLIENT_SECRET`
- API key for vehicle endpoints (`VCC_API_KEY`)

## Install and Run

```bash
cargo build
cargo run -- --help
```

## Quick Start

1. Log in (bridge-managed credentials):

```bash
export VOLVO_AUTH_BRIDGE_URL="https://<your-bridge-domain>"
cargo run -- auth login --auth-bridge-url "$VOLVO_AUTH_BRIDGE_URL"
```

2. Add a VIN and set it as default:

```bash
cargo run -- vehicle vin add --vin "<VIN>" --default
```

3. Fetch vehicle data:

```bash
cargo run -- vehicle windows get --api-key "$VCC_API_KEY"
```

## Common Commands

```bash
# List vehicles
cargo run -- vehicle list --api-key "$VCC_API_KEY"

# Show active identity/session state
cargo run -- auth whoami

# List stored VINs
cargo run -- vehicle vin list

# Get latest known location
cargo run -- location get --api-key "$VCC_API_KEY"

# Run tests
cargo test
```

## Notes

- Command output is JSON by default.
- Configuration is stored locally per profile in SQLite.
- In bridge mode, `client_id` and `client_secret` are configured on the bridge service only.

## VC Cli Auth Service

This repository includes a Cloudflare implementation under `bridge/cloudflare` that provides:

- `GET /` basic homepage
- `GET /privacy.html` privacy notice
- `GET /terms.html` terms and conditions
- `POST /v1/oauth/start` start bridge login session
- `GET /oauth/callback` Volvo callback that hands off to localhost
- `POST /v1/oauth/refresh` refresh via bridge credentials

Deploy and configure:

```bash
cd bridge/cloudflare
npm install
cp wrangler.example.jsonc wrangler.jsonc
npx wrangler kv namespace create OAUTH_SESSIONS
npx wrangler secret put VOLVO_CLIENT_ID
npx wrangler secret put VOLVO_CLIENT_SECRET
npm run deploy
```

For local development or fallback, legacy direct login still works:

```bash
cargo run -- auth login \
  --client-id "$VOLVO_CLIENT_ID" \
  --client-secret "$VOLVO_CLIENT_SECRET"
```

## License

MIT
