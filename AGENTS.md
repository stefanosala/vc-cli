# AGENTS.md

## Purpose

This file tells coding agents where to look first for implementation examples and source-of-truth docs while working on `vc-cli` (Volvo Cars API CLI).

## Primary References

### 1) CLI architecture and patterns

- Focus areas to mirror:
  - command tree conventions (`clap`-style UX)
  - auth command ergonomics
  - JSON-first output style
  - configuration/env precedence patterns

### 2) API docs (source of truth)

- Connected Vehicle API v2 overview: <https://developer.volvocars.com/apis/connected-vehicle/v2/overview/>
- Connected Vehicle API v2 details: <https://developer.volvocars.com/apis/connected-vehicle/v2/details/>
- Doors / windows / locks endpoint family: <https://developer.volvocars.com/apis/connected-vehicle/v2/endpoints/doors-windows-locks/>
- Authorization docs: <https://developer.volvocars.com/apis/docs/authorisation/>

### 3) Sample implementations

- Volvo API samples repository: <https://github.com/volvo-cars/developer-portal-api-samples>
- OAuth code flow sample: `oauth2-code-flow-sample/server.js`
- Connected Vehicle fetch sample: `connected-vehicle-fetch-sample/index.js`

## Local Project Map

When implementing features, start here:

- CLI routing and command wiring: `src/cli/mod.rs`
- Auth login bridge handoff logic: `src/commands/auth_login.rs`
- Auth identity/session commands: `src/commands/auth_whoami.rs`, `src/commands/auth_logout.rs`
- Vehicle windows command: `src/commands/vehicle_windows_get.rs`
- VIN management commands: `src/commands/vehicle_vin.rs`
- HTTP client code: `src/http/client.rs`
- SQLite store and profile/session/VIN persistence: `src/store/sqlite.rs`
- SQLite schema migration: `src/store/migrations/0001_init.sql`

## Endpoint and Environment Defaults

- Connected Vehicle API base URL default: `https://api.volvocars.com`
- Auth issuer default: `https://volvoid.eu.volvocars.com`
- Runtime API host precedence:
  1. `--api-host`
  2. `VOLVO_API_HOST`
  3. active profile value in SQLite
  4. built-in default (`https://api.volvocars.com`)
- API key precedence for vehicle calls:
  1. `--api-key`
  2. `VCC_API_KEY`

## VIN Management Rules

- Persist API-discovered VINs per profile in SQLite.
- Support multiple VINs per profile.
- Keep one default VIN per profile.
- Do not allow manual VIN insertion; VINs must come from the vehicle list endpoint.
- VIN resolution precedence for vehicle commands:
  1. explicit `--vin`
  2. stored default VIN
  3. fetch `vehicle list`, cache discovered VINs, and auto-set the single VIN as default
  4. if multiple VINs are available, prompt the user to choose and persist a default

## Validation Checklist for New Changes

After implementing a change, verify:

1. `cargo test` passes
2. `auth login` succeeds with valid Volvo credentials
3. `vehicle list` discovers VINs from the API
4. `vehicle vin default --vin <VIN>` stores a discovered VIN as default
5. `vehicle windows get` works both with `--vin` and without `--vin` when default VIN exists
6. profile/env precedence behaves as expected

## Notes for Agents

- Prefer updating existing command patterns rather than creating one-off behavior.
- Keep output machine-readable JSON unless a human-readable mode is explicitly requested.
- Treat API docs and endpoint behavior as authoritative when docs and code differ.
- When API docs are inaccessible from the runtime environment, do not guess silently; leave clear validation points and verify against docs before release.

## Local Private Overrides

- Keep this tracked `AGENTS.md` safe for public release.
- Put private repository links, internal notes, and environment-specific instructions in an untracked `AGENTS.private.md`.
- Local tooling can merge `AGENTS.md` with `AGENTS.private.md` when that private file exists.

## Build / test / lint

- **Build:** `cargo build`
- **Test:** `cargo test`
- **Lint:** `cargo clippy`
- **Format check:** `cargo fmt -- --check`
- **Run:** `cargo run -- <subcommand>` (for example: `cargo run -- vehicle list`)
