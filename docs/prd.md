# crgx — npx/uvx for Rust (cargo execute)

## Problem

The Rust ecosystem has no equivalent of `npx` (Node.js) or `uvx` (Python) — a tool that downloads and runs a Rust binary in one command without permanent installation.

Today, running a Rust CLI tool requires:

```bash
# Option A: compile from source (minutes)
cargo install some-tool
some-tool --help

# Option B: cargo-binstall (seconds, but still permanent)
cargo binstall some-tool
some-tool --help
```

Both approaches permanently install the binary. There is no "just run it once" workflow. This is a meaningful gap:

- **MCP servers** can't auto-install like `npx -y some-mcp-server` or `uvx some-mcp-server`
- **CI/CD scripts** install tools they only need once
- **Trying out tools** requires committing to a permanent install
- **One-off commands** (`crgx tokei`, `crgx cargo-expand`) shouldn't require install ceremonies

## Solution

`crgx` — download and run a Rust binary in a single command:

```bash
crgx fossil-mcp scan ./project
crgx tokei .
crgx cargo-expand --help
```

On first run, `crgx` downloads the pre-built binary (via cargo-binstall registry/GitHub releases), caches it, and executes it. Subsequent runs use the cache. No permanent PATH pollution.

## Core Behavior

```
crgx [-y] [--allow-build] [--bin <name>] <crate-name>[@<version>] [args...]
```

1. Check local cache for `<crate-name>` at requested version
2. If not cached: prompt for confirmation (unless `-y` or previously confirmed)
3. Resolve crate metadata from crates.io, auto-detect binary name from `[[bin]]` targets
4. Download pre-built binary (prefer binstall metadata → GitHub releases)
5. If no binary found: fail with message (or compile from source if `--allow-build`)
6. Cache the binary and execute with `[args...]`, forwarding stdin/stdout/stderr and exit code

### Version Pinning

```bash
crgx fossil-mcp@0.1.2 scan ./project   # exact version
crgx fossil-mcp@latest scan ./project   # force latest (default)
```

### Cache Management

```bash
crgx --cache-list          # show cached binaries
crgx --cache-clean         # remove all cached binaries
crgx --cache-dir           # print cache directory path
```

Cache location: `~/.cache/crgx/` (Linux), `~/Library/Caches/crgx/` (macOS), `%LOCALAPPDATA%\crgx\cache` (Windows).

## Binary Resolution Strategy

Priority order for finding pre-built binaries:

1. **cargo-binstall metadata** in Cargo.toml (`[package.metadata.binstall]`) — this is the richest source; many crates already have this configured
2. **GitHub Releases** — detect repo from crates.io metadata, match asset names by target triple and common naming patterns
3. **Fallback: compile from source** — `cargo install` into the cache directory (only with `--allow-build` flag; without it, crgx fails fast)

This means any crate that already supports `cargo binstall` automatically works with `crgx` at full speed.

## Target Use Cases

### 1. MCP Server Configuration

The killer use case. Today:
```json
{"command": "fossil-mcp"}
```
Requires the binary to already be installed. With crgx:
```json
{"command": "crgx", "args": ["-y", "fossil-mcp"]}
```
Auto-downloads on first run — zero install step, like `npx` for Node MCP servers.

### 2. CI/CD

```yaml
- run: crgx cargo-tarpaulin --out xml
```

No `cargo install` step burning CI minutes.

### 3. One-off Tool Usage

```bash
crgx cargo-expand src/main.rs      # macro expansion
crgx tokei .                        # line counts
crgx cargo-audit                    # security audit
```

### 4. Scripting

```bash
#!/bin/bash
crgx fossil-mcp scan "$1" --format sarif -o results.sarif
```

## Technical Design

### Language & Distribution

- Written in Rust (single static binary, no runtime deps)
- Distributed via crates.io (`cargo install crgx`) and pre-built binaries
- Bootstrapping: `crgx` itself should be trivially installable — the whole point is reducing install friction

### Architecture

```
crgx <crate>@<version> [args...]
  |
  v
[Cache lookup] --hit--> [Execute binary]
  |
  miss
  v
[Resolve crate metadata from crates.io API]
  |
  v
[Find binary: binstall metadata → GitHub releases → fail (or cargo install with --allow-build)]
  |
  v
[Download/compile → store in cache]
  |
  v
[Execute binary, forward stdio]
```

### Key Dependencies

- **crates.io API** — crate metadata, repo URL, version resolution, `[[bin]]` target discovery
- **Vendored binstall resolution** — reimplement binstall metadata parsing and asset URL template resolution directly in crgx
- **GitHub Releases API** — asset discovery fallback
- **Platform detection** — target triple for binary matching

### Cache Structure

```
~/.cache/crgx/
  bin/
    fossil-mcp/
      0.1.2/
        fossil-mcp         # the binary
        metadata.json       # version, source, timestamp
    tokei/
      13.0.0/
        tokei
        metadata.json
```

### Update Behavior

- `crgx some-tool` uses the cached version if available (fast)
- `crgx some-tool@latest` checks for updates (network call, slower)
- Stale cache: check for updates periodically (default: 24h), configurable

## Non-Goals (v1)

- **Package management** — crgx runs binaries, it doesn't manage a project's dependencies
- **Library usage** — only CLI binaries, not `use some_crate;` in Rust code
- **Shell completion for wrapped tools** — out of scope for v1
- **Custom registries** — crates.io only for v1

## Success Metrics

- Time to first run: <3 seconds for a crate with binstall metadata (download + execute)
- Cache hit execution: <50ms overhead vs running the binary directly
- Works with any crate that has binstall metadata — zero config on the crate author's side

## Prior Art

| Tool | Ecosystem | How it works |
|------|-----------|-------------|
| `npx` | Node.js | Downloads from npm, caches in node_modules/.cache, runs |
| `uvx` | Python | Downloads from PyPI, creates ephemeral venv, runs |
| `bunx` | Bun/Node | Like npx but faster, uses Bun's global cache |
| `pipx run` | Python | Downloads, creates temp venv, runs, cleans up |

`crgx` fills the same gap for Rust.

## Decisions

1. **Vendor binstall resolution logic.** Single static binary with zero external dependencies. No requirement for `cargo-binstall` to be installed. Reimplement the resolution strategy (binstall metadata parsing, GitHub Releases asset matching) directly in crgx.

2. **Fallback compilation is opt-in via `--allow-build`.** If no pre-built binary is found, crgx fails fast with a clear message: `"No pre-built binary found for <crate>. Run with --allow-build to compile from source."` This avoids surprise multi-minute waits.

3. **Auto-resolve binary name from crates.io metadata.** Read `[[bin]]` targets from crate metadata to determine the correct binary name. If a crate produces multiple binaries, default to the one matching the crate name; if none matches, use the first `[[bin]]` entry. Add `--bin <name>` flag as an override.

4. **Prompt on first run, `-y` to skip.** On first run of a crate not previously cached, crgx prompts for confirmation: `"Download and run <crate> v<version>? [y/N]"`. The `-y` flag auto-confirms (for CI/scripts/MCP configs). Example: `crgx -y fossil-mcp scan ./project`.
