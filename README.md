# crgx

[![CI](https://github.com/yfedoseev/crgx/actions/workflows/ci.yml/badge.svg)](https://github.com/yfedoseev/crgx/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/crgx.svg)](https://crates.io/crates/crgx)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Run any crate binary instantly** — no cargo install, no compile wait, no ceremony.

```bash
crgx tokei .
```

crgx is the [npx](https://docs.npmjs.com/cli/commands/npx) equivalent for the Rust ecosystem. It fetches pre-built binaries, caches them locally, and executes them in a single command — no compilation required. Any crate that works with [cargo-binstall](https://github.com/cargo-bins/cargo-binstall) works with crgx automatically.

## Why crgx?

- **No compilation** — downloads pre-built binaries in seconds instead of compiling for minutes
- **No global install** — binaries live in crgx's own cache, not `~/.cargo/bin`
- **One command** — `crgx tokei .` resolves, downloads, caches, and runs
- **Works offline** — falls back to cached binaries when the network is unavailable
- **MCP-ready** — drop `crgx -y <server>` into any MCP config with zero setup

## How does crgx compare?

| Tool | What it does | Compile? | Global install? | One command? |
|------|-------------|----------|-----------------|-------------|
| **crgx** | Fetch + cache + run pre-built binary | No | No (own cache) | Yes |
| `cargo install` | Compile from source, install to ~/.cargo/bin | Yes (slow) | Yes | Yes |
| `cargo binstall` | Install pre-built binary to ~/.cargo/bin | No | Yes | Yes |
| npx (Node.js) | Run npm package binary | N/A | No | Yes |

## Install

| Platform | Command |
|----------|---------|
| macOS (Homebrew) | `brew install yfedoseev/tap/crgx` |
| Linux / macOS (shell) | `curl -fsSL crgx.dev/install.sh \| sh` |
| Windows (PowerShell) | `irm crgx.dev/install.ps1 \| iex` |
| Any (Cargo) | `cargo install crgx` |

Or download pre-built binaries from [GitHub Releases](https://github.com/yfedoseev/crgx/releases).

## Usage

```
crgx [FLAGS] <crate>[@<version>] [tool-args...]
```

Everything after the crate specifier is passed directly to the tool:

```bash
crgx tokei .                         # count lines of code
crgx ripgrep -- -i "pattern" src/    # search files
crgx cargo-audit                     # security audit
crgx -y fossil-mcp                   # skip download prompt
```

### Version pinning

```bash
crgx tokei@14.0.0 .          # exact version (cached forever)
crgx tokei@latest .           # force latest from registry
crgx tokei .                  # latest, with 24h staleness check
```

### MCP servers

```json
{
  "command": "crgx",
  "args": ["-y", "fossil-mcp"]
}
```

Drop into any MCP config. No pre-install step.

### CI/CD

```yaml
- run: crgx -y cargo-tarpaulin -- --out xml
```

No `cargo install` step burning CI minutes.

## Flags

| Flag | Description |
|------|-------------|
| `-y`, `--yes` | Auto-confirm download prompts |
| `--allow-build` | Allow compiling from source if no pre-built binary found |
| `--bin <name>` | Specify which binary to run (for multi-binary crates) |
| `--no-install` | Only run if already cached; fail otherwise |

## Cache management

```bash
crgx --cache-list             # show cached binaries
crgx --cache-clean            # remove all cached binaries
crgx --cache-dir              # print cache directory path
```

| OS | Cache path |
|----|-----------|
| Linux | `~/.cache/crgx/` |
| macOS | `~/Library/Caches/crgx/` |
| Windows | `%LOCALAPPDATA%\crgx\cache` |

Exact versions (`tool@1.2.3`) are cached forever. Unversioned calls use a 24h staleness window.

## Resolution chain

crgx tries each source in order to find a pre-built binary:

1. **Local cache** — instant if already downloaded
2. **Binstall metadata** — reads the crate author's download URL from Cargo.toml metadata
3. **GitHub Releases** — matches a release asset by your target triple
4. **QuickInstall** — falls back to cargo-quickinstall's pre-built binary registry
5. **cargo build** — opt-in with `--allow-build`, compiles from source as a last resort

Any crate that works with `cargo binstall` works with crgx automatically.

## How it works

On first run, crgx resolves the crate from crates.io, downloads a pre-built binary, caches it, and `exec()`s it. The crgx process is replaced entirely — zero overhead, signals pass through, exit codes are preserved.

Subsequent runs hit the cache and execute immediately.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## License

MIT — see [LICENSE](LICENSE) for details.
