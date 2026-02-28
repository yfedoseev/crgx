# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-02-28

> Initial release

### New Features

- **Core CLI**: `crgx <crate>[@<version>] [tool-args...]` — run any crate binary in one command
- **Binary resolution chain**: Local cache → Binstall metadata → GitHub Releases → QuickInstall → cargo build (opt-in)
- **Version pinning**: Exact (`@1.2.3`), latest (`@latest`), or bare name with 24h staleness window
- **Cache management**: `--cache-list`, `--cache-clean`, `--cache-dir`
- **Quiet by default** — no status output unless `--verbose` is passed
- **Downloads automatically** without prompting (like uvx)
- **Flags**: `-v`/`--verbose`, `--allow-build`, `--bin <name>`, `--offline`
- **`--offline` flag** — only run if already cached; no network access
- **`--verbose` flag** — show detailed resolution and download progress
- **Network resilience**: HTTP timeouts (10s connect, 60s download), automatic retries with backoff, offline cache fallback for previously downloaded binaries
- **Cross-platform**: Linux, macOS, and Windows support
- **Zero overhead**: Uses `exec()` to replace the crgx process entirely — signals pass through, exit codes are preserved
- **Binstall compatibility**: Full support for `[package.metadata.binstall]` template variables including `{ repo }`, `{ target }`, `{ archive-suffix }`, `{ binary-ext }`, target part variables, and per-target overrides
- **Archive support**: Handles tar.gz, tar.xz, and zip archives automatically
- **MCP server support**: Use `crgx <server>` in any MCP config for zero-setup tool servers

### Distribution

- Homebrew tap (`brew install yfedoseev/tap/crgx`)
- Cargo (`cargo install crgx`)
- Shell installer (`curl -fsSL crgx.dev/install.sh | sh`)
- PowerShell installer (`irm crgx.dev/install.ps1 | iex`)
- Pre-built binaries for 6 targets via GitHub Releases
- Website with docs at [crgx.dev](https://crgx.dev)

[0.1.0]: https://github.com/yfedoseev/crgx/releases/tag/v0.1.0
