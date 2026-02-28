# crgx

npx for Rust — download and run any crate binary in one command.

```bash
$ crgx tokei .
```

No `cargo install`. No permanent binaries. No ceremony.

## Install

```bash
brew install crgx
```

```bash
cargo install crgx
```

```bash
curl -fsSL crgx.dev/install.sh | sh
```

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

Cache location:
- Linux: `~/.cache/crgx/`
- macOS: `~/Library/Caches/crgx/`
- Windows: `%LOCALAPPDATA%\crgx\cache`

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

## License

MIT
