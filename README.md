# OpenAPI Validator TUI

[![GitHub Release](https://img.shields.io/github/v/release/entur/openapi-validator-tui?style=flat-square&label=release)](https://github.com/entur/openapi-validator-tui/releases/latest)
[![Rust](https://img.shields.io/badge/rust-1.92%2B-orange?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![Homebrew](https://img.shields.io/github/v/release/entur/openapi-validator-tui?style=flat-square&label=homebrew&color=fbb040)](https://github.com/entur/openapi-validator-tui#homebrew)
[![License](https://img.shields.io/badge/license-EUPL--1.2-blue?style=flat-square)](LICENSE.md)
[![Issues](https://img.shields.io/github/issues/entur/openapi-validator-tui?style=flat-square)](https://github.com/entur/openapi-validator-tui/issues)
[![Pull Requests](https://img.shields.io/github/issues-pr/entur/openapi-validator-tui?style=flat-square)](https://github.com/entur/openapi-validator-tui/pulls)
[![Last Commit](https://img.shields.io/github/last-commit/entur/openapi-validator-tui?style=flat-square)](https://github.com/entur/openapi-validator-tui/commits/main)

Interactive terminal UI for linting, generating, and compiling OpenAPI specs. Explore validation results, browse generated code, compare diffs across runs, and edit specs — all from a single TUI powered by Docker and `.oavc` config.

## Quick start

```bash
cd your-project/
lazyoav
```

Launches the TUI in the current directory. Reads `.oavc` for config and runs the lint/generate/compile pipeline interactively.

## Install

### Homebrew

```bash
brew tap entur/openapi-validator-tui https://github.com/entur/openapi-validator-tui
brew install lazyoav
```

### Shell script

```bash
curl -fsSL https://raw.githubusercontent.com/entur/openapi-validator-tui/main/install.sh | bash
```

### Cargo

```bash
cargo install --git https://github.com/entur/openapi-validator-tui
```

### Uninstall

| Method   | Command                          |
|----------|----------------------------------|
| Homebrew | `brew uninstall lazyoav`         |
| Cargo    | `cargo uninstall lazyoav`        |
| Manual   | `rm /usr/local/bin/lazyoav`      |

## Features

| Feature | Description |
|---------|-------------|
| Validation pipeline | Lint, generate, and compile OpenAPI specs via Docker |
| Spec browser | Navigate and search your spec with syntax highlighting |
| Generated code browser | Explore code output per generator |
| Diff view | Compare generated code across pipeline runs |
| External editor | Open spec in `$EDITOR` directly from the TUI |
| Configurable keybindings | Remap keys via `.oavc` config |
| Custom generators | Define generators via YAML in `.oav/generators/` |

## Keybindings

Default keybindings (configurable via `.oavc`):

| Key | Action |
|-----|--------|
| `q` | Quit |
| `r` | Run validation pipeline |
| `e` | Open spec in external editor |
| `Tab` | Cycle panels |
| `j/k` or arrows | Navigate lists |
| `Enter` | Select / expand |
| `?` | Toggle help overlay |

## Config

The TUI reads the same `.oavc` config as the CLI. Minimal example:

```yaml
spec: openapi/api.yaml
mode: both
linter: spectral
```

See the [CLI documentation](https://github.com/entur/openapi-validator-cli) for the full config reference.

## Requirements

- Docker (for linting, generation, and compile steps)

## Build

```bash
cargo build --release
```

## Testing

Integration tests require Docker.

```bash
cargo test -- --ignored
```
