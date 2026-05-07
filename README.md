# VC Cli

VC Cli is a command-line interface for interacting with the Volvo Cars Connected Vehicle API.

## Prerequisites

- Rust toolchain (stable)
- Optional custom auth bridge (`VOLVO_AUTH_BRIDGE_URL`); defaults to `https://vc-cli.com`
- API key for vehicle endpoints (`VCC_API_KEY`)

## Install and Run

```bash
cargo build
cargo run -- --help
```

Global npm install (after package is published):

```bash
npm i -g vc-cli
vc-cli --help
```

## Quick Start

1. Log in with the default auth bridge (`https://vc-cli.com`):

```bash
cargo run -- auth login
```

2. Discover your vehicles:

```bash
cargo run -- vehicle list --api-key "$VCC_API_KEY"
```

3. Set a default VIN if more than one vehicle is available:

```bash
cargo run -- vehicle vin default --vin "<VIN>" --api-key "$VCC_API_KEY"
```

4. Fetch vehicle data:

```bash
cargo run -- vehicle windows get --api-key "$VCC_API_KEY"
```

If no default VIN is configured, VIN-based commands discover vehicles automatically. A single VIN is saved as the default; multiple VINs are shown as an interactive prompt.

## Common Commands

```bash
# List vehicles
cargo run -- vehicle list --api-key "$VCC_API_KEY"

# Show active identity/session state
cargo run -- auth whoami

# Get latest known location
cargo run -- location get --api-key "$VCC_API_KEY"

# Run tests
cargo test
```

## Notes

- Command output is JSON by default.
- Configuration is stored locally per profile in SQLite.
- Volvo OAuth `client_id` and `client_secret` are configured on the bridge service only.
- `auth login` requests all Connected Vehicle, Energy, and Location scopes by default. Use `--scopes` or `VOLVO_SCOPES` to override the requested scope set.

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

## npm Release Flow

This repository now mirrors the Day One npm distribution model:

- `.github/workflows/build-binaries.yml` builds tagged binaries and attaches `vc-cli-<platform>` assets to the GitHub release.
- `.github/workflows/release-npm.yml` downloads release assets, updates npm package versions from the tag, and publishes:
  - `vc-cli` (launcher package)
  - `vc-cli-darwin-arm64`
  - `vc-cli-linux-x64`
  - `vc-cli-win32-x64`

Recommended release steps:

1. Push a semantic tag such as `v0.2.0` (triggers binary build + release assets).
2. Run `Release npm` workflow with `tag=v0.2.0` and `dry_run=true`.
3. Re-run with `dry_run=false` once the dry run output looks correct.

## License

MIT
