# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.1.1] - 2026-09-30

### Fixed

- **Fall back to compatible targets when there's no binary for the exact host target** ([#14]). Releases that only ship `x86_64-unknown-linux-musl` (e.g. `cargo-about`) now work on glibc Linux. A musl build of crgx (the Homebrew Linux build) falls back to `-gnu` binaries when the system has glibc; Alpine's `gcompat` shim doesn't count as glibc. macOS falls back to universal binaries, plus x86_64 on Apple Silicon when Rosetta is installed. Windows MSVC falls back to `-gnu`. The fallback applies to binstall metadata, GitHub Releases and QuickInstall. Cache metadata records the target that was actually downloaded, and `--verbose` says when a fallback was used.
- **SOCKS proxies from `ALL_PROXY` / `HTTPS_PROXY` were silently ignored** ([#13]). SOCKS4, SOCKS4a, SOCKS5 and SOCKS5h now work.
- **Downloads larger than 10 MiB failed** with "response body is larger than request limit". The limit is now 1 GiB.
- The "no pre-built binary found" error now lists every target that was tried.

### Added

- `-F`/`--features <list>`, `--no-default-features` and `--all-features` for building from source ([#14]). These imply a source build, and the result is cached separately from the pre-built binary.
- The Windows system proxy is used when no proxy environment variable is set.
- `--verbose` prints the proxy in use (without credentials).
- `GITHUB_TOKEN` / `GH_TOKEN` are used for GitHub API requests, which raises the rate limit from 60 to 5000 requests/hour. The token is only sent to `api.github.com`.
- `--help` documents the proxy environment variables.
- `CRGX_CACHE_DIR` and upstream URL overrides (`CRGX_CRATES_IO_API`, `CRGX_CRATES_STATIC`, `CRGX_GITHUB_API`, `CRGX_QUICKINSTALL_URL`), used by the end-to-end tests.

### Security

- Updated all dependencies. 0.1.0 shipped with these advisories, which are fixed here:
  - `rustls` 0.23.37: RUSTSEC-2026-0285
  - `rustls-webpki` 0.103.9: RUSTSEC-2026-0049, RUSTSEC-2026-0098, RUSTSEC-2026-0099, RUSTSEC-2026-0104
  - `tar` 0.4.44: RUSTSEC-2026-0068 (PAX size headers ignored). RUSTSEC-2026-0067 only affects `unpack`, which crgx doesn't use.
- Crate names are now checked against crates.io's rules before use. Before, a specifier such as `../../x@1.0.0` made crgx look for, and run, a "cached" binary outside its cache directory.
- Replaced the unmaintained `xz2` with `liblzma`, and upgraded `dirs` to 7.
- Removed unused dependencies (`clap`, `anyhow`, `url`, and the dev-dependency `mockito`). This also drops `anyhow`'s RUSTSEC-2026-0190 and the dev-only `h2`/`rand` advisories.
- CI now runs `cargo audit` weekly and fails on unmaintained, unsound or yanked crates as well as vulnerabilities.

### Testing

- New end-to-end suite that runs the real binary against a mock crates.io, GitHub and QuickInstall. It covers target fallback, SOCKS5/SOCKS5h/HTTP proxies, `NO_PROXY`, feature builds with a stub `cargo`, caching and `.tar.xz` archives.
- New CI jobs: a musl build on a glibc host, Alpine with and without `gcompat`, and network smoke tests on Linux, macOS and Windows against the real crates.io and GitHub (including through SOCKS5h and HTTP CONNECT proxies).

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

[0.1.1]: https://github.com/yfedoseev/crgx/releases/tag/v0.1.1
[0.1.0]: https://github.com/yfedoseev/crgx/releases/tag/v0.1.0
[#13]: https://github.com/yfedoseev/crgx/issues/13
[#14]: https://github.com/yfedoseev/crgx/issues/14
