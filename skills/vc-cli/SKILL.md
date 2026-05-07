---
name: vc-cli
description: "VC CLI: auth, energy, location, and vehicle command workflows for the Volvo Cars Connected Vehicle API."
allowed-tools: "Bash(vc-cli:*)"
---

# vc-cli

Use this skill when operating `vc-cli` commands across authentication, vehicle reads, location, energy, and remote vehicle command invocation.

## Usage

```bash
vc-cli <command> [flags]
```

For local development without installing the binary:

```bash
cargo run -- <command> [flags]
```

## Command Map

- `auth login` - Start browser-based OAuth login and persist session tokens.
- `auth token-set` - Persist tokens directly (script/manual token flow).
- `auth whoami` - Print current session identity info.
- `auth logout` - Clear local auth session.

- `energy state get` - Fetch current energy state for a VIN.
- `energy capabilities get` - Fetch energy capability metadata for a VIN.

- `location get` - Fetch latest known vehicle location for a VIN.

- `vehicle list` - List vehicles available to the authenticated account.
- `vehicle details get` - Fetch vehicle details.
- `vehicle windows get` - Fetch windows state.
- `vehicle doors get` - Fetch doors/locks state.
- `vehicle warnings get` - Fetch warning indicators.
- `vehicle tyres get` - Fetch tyre status.
- `vehicle statistics get` - Fetch trip/usage statistics.
- `vehicle odometer get` - Fetch odometer state.
- `vehicle fuel get` - Fetch fuel state.
- `vehicle diagnostics get` - Fetch diagnostics overview.
- `vehicle engine diagnostics get` - Fetch engine diagnostics.
- `vehicle engine status get` - Fetch engine status.
- `vehicle brakes get` - Fetch brakes state.
- `vehicle vin default` - Set default VIN for the active profile.

- `vehicle commands list` - List command statuses/history.
- `vehicle commands accessibility` - Fetch accessibility/state for remote commands.
- `vehicle commands unlock` - Invoke unlock.
- `vehicle commands lock` - Invoke lock.
- `vehicle commands lock-reduced-guard` - Invoke reduced-guard lock.
- `vehicle commands honk` - Invoke horn.
- `vehicle commands flash` - Invoke lights flash.
- `vehicle commands honk-flash` - Invoke horn+flash.
- `vehicle commands engine-start` - Start engine with runtime.
- `vehicle commands engine-stop` - Stop engine.
- `vehicle commands climatization-start` - Start climatization.
- `vehicle commands climatization-stop` - Stop climatization.

## Global Flags and Precedence

| Flag | Description |
|------|-------------|
| `--api-host` | One-off API base host override |
| `--profile` | Profile name used for local state/session/VIN resolution |

Runtime API host precedence:

1. `--api-host`
2. `VOLVO_API_HOST`
3. Active profile value in SQLite
4. Built-in default (`https://api.volvocars.com`)

Vehicle API key precedence:

1. `--api-key`
2. `VCC_API_KEY`

## VIN Resolution

VIN-based commands resolve VIN in this order:

1. Explicit `--vin`
2. Stored default VIN for active profile
3. `vehicle list` discovery and cache (auto-set if only one VIN exists)
4. User prompt to choose a default when multiple VINs are discovered

VINs should come from API discovery (`vehicle list`); do not rely on manual VIN insertion flows.
When multiple VINs are discovered in non-interactive contexts (stdin is not a TTY), resolution fails with an actionable error; pass `--vin` or preconfigure a default VIN first.

Use this command to set/replace the profile default:

```bash
vc-cli vehicle vin default --vin <VIN> --api-key "$VCC_API_KEY"
```

## Command Details

### `auth login`

```bash
vc-cli auth login [--scopes <space-separated-scopes>] [--auth-bridge-url <url>] [--auth-listen-timeout-seconds <seconds>]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--scopes` | no | Scope set to request (defaults to full Connected Vehicle + Energy + Location scopes) |
| `--auth-bridge-url` | no | Auth bridge URL (`VOLVO_AUTH_BRIDGE_URL` default is `https://vc-cli.com`) |
| `--auth-listen-timeout-seconds` | no | Local callback listener timeout |

Example:

```bash
vc-cli auth login
vc-cli auth login --auth-bridge-url https://vc-cli.com
```

### `auth token-set`

```bash
vc-cli auth token-set --access-token <token> [--refresh-token <token>] [--expires-in <seconds>] [--scope <scopes>] [--token-type <type>] [--token-endpoint <url>]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--access-token` | yes | Access token to persist |
| `--refresh-token` | no | Refresh token |
| `--expires-in` | no | TTL in seconds |
| `--scope` | no | Scope string |
| `--token-type` | no | Token type (`Bearer` default) |
| `--token-endpoint` | no | Source token endpoint URL |

Prefer environment variables for secret values to reduce shell-history/process-listing exposure:

```bash
VOLVO_ACCESS_TOKEN="<token>" VOLVO_REFRESH_TOKEN="<token>" vc-cli auth token-set --access-token "$VOLVO_ACCESS_TOKEN" --refresh-token "$VOLVO_REFRESH_TOKEN"
```

### VIN/API-key vehicle read commands

Most vehicle and energy/location read commands use:

```bash
--vin <VIN> --api-key <KEY>
```

Both are optional in the parser, but command execution requires a resolvable VIN and API key by precedence.

### `vehicle commands engine-start`

```bash
vc-cli vehicle commands engine-start --runtime-minutes <minutes> [--vin <VIN>] [--api-key <KEY>]
```

`--runtime-minutes` is required.

## Discovering Commands

```bash
vc-cli --help
vc-cli auth --help
vc-cli energy --help
vc-cli location --help
vc-cli vehicle --help
vc-cli vehicle commands --help
```

## Examples

```bash
# Auth
vc-cli auth login
vc-cli auth whoami

# Vehicle list and default VIN
vc-cli vehicle list --api-key "$VCC_API_KEY"
vc-cli vehicle vin default --vin "<VIN>" --api-key "$VCC_API_KEY"

# Vehicle data reads
vc-cli vehicle windows get --api-key "$VCC_API_KEY"
vc-cli location get --api-key "$VCC_API_KEY"
vc-cli energy state get --api-key "$VCC_API_KEY"

# Vehicle command invocation
vc-cli vehicle commands lock --api-key "$VCC_API_KEY"
vc-cli vehicle commands engine-start --runtime-minutes 10 --api-key "$VCC_API_KEY"
```

## Output and Safety Rules

- Output is JSON-first.
- Never print or store sensitive token/API key values in logs or transcripts.
- For `auth token-set`, prefer env vars (`VOLVO_ACCESS_TOKEN`, `VOLVO_REFRESH_TOKEN`, `VOLVO_TOKEN_ENDPOINT`) over raw literal tokens in shell command history.
- Treat `vehicle commands *` as write-like remote actions and require explicit user intent before invoking.
- Prefer `--api-host` for one-off calls and `--profile` when switching persistent local context.
