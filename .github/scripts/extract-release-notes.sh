#!/bin/bash
# Extracts release notes for a given version from CHANGELOG.md
# Usage: extract-release-notes.sh <version>
# Outputs:
#   release-title.txt  — "v0.1.0 | Initial release"
#   release-notes.md   — Full body (changelog section + installation footer)

set -euo pipefail

VERSION="$1"
CHANGELOG="CHANGELOG.md"

if [ ! -f "$CHANGELOG" ]; then
  echo "Error: $CHANGELOG not found" >&2
  exit 1
fi

# Extract subtitle from "> ..." line after version header
SUBTITLE=$(awk "/^## \[${VERSION}\]/{found=1; next} found && /^>/{gsub(/^> */, \"\"); print; exit}" "$CHANGELOG")

# Build title
if [ -n "$SUBTITLE" ]; then
  echo "v${VERSION} | ${SUBTITLE}" > release-title.txt
else
  echo "v${VERSION}" > release-title.txt
fi

# Extract body: everything between this version's ## and the next ##
awk "/^## \[${VERSION}\]/{flag=1; next} /^## \[/{flag=0} flag" "$CHANGELOG" \
  | sed '/^> /d' \
  | sed '1{/^$/d}' > changelog-section.md

if [ ! -s changelog-section.md ]; then
  echo "Warning: No changelog content found for version ${VERSION}" >&2
fi

# Build release body = changelog section + installation footer
cat changelog-section.md > release-notes.md
cat >> release-notes.md << 'FOOTER'

---

### Installation

**Homebrew**
```bash
brew install yfedoseev/tap/crgx
```

**Cargo**
```bash
cargo install crgx
```

**Shell (Linux / macOS)**
```bash
curl -fsSL crgx.dev/install.sh | sh
```

**PowerShell (Windows)**
```powershell
irm crgx.dev/install.ps1 | iex
```

**Pre-built Binaries**
Download archives for your platform from the assets below.

### Platform Support
| Platform | Architecture | Archive |
|----------|-------------|---------|
| Linux | x86_64 (glibc) | `crgx-linux-x86_64-*.tar.gz` |
| Linux | x86_64 (musl) | `crgx-linux-x86_64-musl-*.tar.gz` |
| Linux | ARM64 | `crgx-linux-aarch64-*.tar.gz` |
| macOS | x86_64 (Intel) | `crgx-macos-x86_64-*.tar.gz` |
| macOS | ARM64 (Apple Silicon) | `crgx-macos-aarch64-*.tar.gz` |
| Windows | x86_64 | `crgx-windows-x86_64-*.zip` |

### Changelog
See [CHANGELOG.md](https://github.com/yfedoseev/crgx/blob/main/CHANGELOG.md) for full details.
FOOTER

# Cleanup
rm -f changelog-section.md

echo "Generated release-title.txt and release-notes.md for v${VERSION}"
