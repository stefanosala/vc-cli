# VC Cli

VC Cli is a command-line interface for interacting with the Volvo Cars Connected Vehicle API.

## Prerequisites

- Rust toolchain (stable)
- Volvo OAuth client credentials (`VOLVO_CLIENT_ID`, `VOLVO_CLIENT_SECRET`); prompted on first `auth login`
- Registered OAuth redirect URI; defaults to `http://127.0.0.1:1410/callback`
- API key for vehicle endpoints (`VCC_API_KEY`); prompted on first `auth login`

## Install and Run


Global npm install:

```bash
npm i -g @stefanosala/vc-cli
vc-cli --help
```

Install the skills for your agent:

```bash
npx skills add stefanosala/vc-cli
```

## Quick Start

1. Log in with Volvo OAuth:

```bash
vc-cli auth login
```

For headless hosts (agents/servers), use:

```bash
vc-cli auth login --headless
```

This prints an authorization URL. Open it in any browser, complete login, then paste the redirected callback URL back into the CLI.

If developer credentials are missing, the command shows the Volvo developer account URL and redirect URI to use, then waits for you to return and enter the API key, client ID, and client secret.

2. Discover your vehicles:

```bash
vc-cli vehicle list
```

3. Set a default VIN if more than one vehicle is available:

```bash
vc-cli vehicle vin default --vin "<VIN>"
```

4. Fetch vehicle data:

```bash
vc-cli vehicle windows get
```

If no default VIN is configured, VIN-based commands discover vehicles automatically. A single VIN is saved as the default; multiple VINs are shown as an interactive prompt.

## Common Commands

```bash
# List vehicles
vc-cli vehicle list

# Show active identity/session state
vc-cli auth whoami

# Get latest known location
vc-cli location get
```

## Notes

- Command output is JSON by default.
- Configuration is loaded from `~/.config/vc-cli/config` in env variable format before CLI parsing.
- Profile/session/VIN state is stored locally per profile in SQLite.
- `auth login` starts a temporary local HTTP listener for the OAuth redirect.
- `auth login --headless` skips browser/listener setup, prints the authorize URL, and prompts for the redirected callback URL.
- `auth login` prompts for missing `VCC_API_KEY`, `VOLVO_CLIENT_ID`, and `VOLVO_CLIENT_SECRET`, then saves them to `~/.config/vc-cli/config`.
- `auth login` requests all Connected Vehicle, Energy, and Location scopes by default. Use `--scopes` or `VOLVO_SCOPES` to override the requested scope set.

## License

MIT
