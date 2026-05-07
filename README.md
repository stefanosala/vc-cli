# VC Cli

VC Cli is a command-line interface for interacting with the Volvo Cars Connected Vehicle API.

## Prerequisites

- Rust toolchain (stable)
- Optional custom auth bridge (`VOLVO_AUTH_BRIDGE_URL`); defaults to `https://vc-cli.com`
- API key for vehicle endpoints (`VCC_API_KEY`)

## Install and Run


Global npm install:

```bash
npm i -g vc-cli
vc-cli --help
```

Install the Cursor skill:

```bash
npx skills add stefanosala/vc-cli
```

## Quick Start

1. Log in with the default auth bridge (`https://vc-cli.com`):

```bash
vc-cli auth login
```

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
- Configuration is stored locally per profile in SQLite.
- Volvo OAuth `client_id` and `client_secret` are configured on the bridge service only.
- `auth login` requests all Connected Vehicle, Energy, and Location scopes by default. Use `--scopes` or `VOLVO_SCOPES` to override the requested scope set.

## License

MIT
