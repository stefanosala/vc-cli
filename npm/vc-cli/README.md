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

## Platform Packages

The launcher depends on one of:

- `@stefanosala/vc-cli-darwin-arm64`
- `@stefanosala/vc-cli-linux-x64`
- `@stefanosala/vc-cli-win32-x64`

If installation is interrupted or the platform package is missing, reinstall:

```bash
npm i -g @stefanosala/vc-cli
```

## Source

Repository: <https://github.com/stefanosala/vc-cli>
