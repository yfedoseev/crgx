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
- **Flags**: `-y`/`--yes`, `--allow-build`, `--bin <name>`, `--no-install`
- **Cross-platform**: Linux, macOS, and Windows support
- **Zero overhead**: Uses `exec()` to replace the crgx process entirely — signals pass through, exit codes are preserved
- **Archive support**: Handles tar.gz, tar.xz, and zip archives automatically
- **Interactive prompts**: Confirmation before downloading, with TTY detection

### Distribution

- Homebrew tap (`brew install yfedoseev/tap/crgx`)
- Cargo (`cargo install crgx`)
- Shell installer (`curl -fsSL crgx.dev/install.sh | sh`)
- PowerShell installer (`irm crgx.dev/install.ps1 | iex`)
- Pre-built binaries for 6 targets via GitHub Releases

[0.1.0]: https://github.com/yfedoseev/crgx/releases/tag/v0.1.0
