# vc-cli

`vc-cli` is a command-line interface for interacting with the Volvo Cars Connected Vehicle API.

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

1. Log in:

```bash
cargo run -- auth login \
  --client-id "$VOLVO_CLIENT_ID" \
  --client-secret "$VOLVO_CLIENT_SECRET"
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

## License

MIT
