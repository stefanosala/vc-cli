# @stefanosala/vc-cli

CLI for the Volvo Cars Connected Vehicle API.

This package is the cross-platform launcher for `vc-cli`. It installs a
platform-specific binary package for your OS/CPU and executes that binary.

## Install

```bash
npm i -g @stefanosala/vc-cli
```

## Usage

```bash
vc-cli --help
vc-cli auth login
vc-cli vehicle list
```

On first login, `vc-cli` may prompt for Volvo developer credentials:

- `VCC_API_KEY`
- `VOLVO_CLIENT_ID`
- `VOLVO_CLIENT_SECRET`

If they are missing, the command shows the Volvo developer account URL and
the redirect URI to configure. Create and publish a new app at
<https://developer.volvocars.com/account/> using the shown redirect URI
(`http://127.0.0.1:1410/callback` by default), then return to the terminal to
finish login.

The CLI loads configuration from `~/.config/vc-cli/config` in env variable
format and saves newly entered credentials there.

## Platform Packages

The launcher depends on one of:

- `@stefanosala/vc-cli-darwin-arm64`
- `@stefanosala/vc-cli-linux-arm64`
- `@stefanosala/vc-cli-linux-x64`
- `@stefanosala/vc-cli-win32-x64`

If installation is interrupted or the platform package is missing, reinstall:

```bash
npm i -g @stefanosala/vc-cli
```

## Source

Repository: <https://github.com/stefanosala/vc-cli>
