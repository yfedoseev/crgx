use std::fmt;
use std::path::Path;

/// Parsed crate specifier: `name`, `name@1.2.3`, or `name@latest`.
#[derive(Debug, Clone)]
pub struct CrateSpec {
    pub name: String,
    pub version: VersionReq,
}

/// Version requirement from the CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionReq {
    /// `crgx tool` — use cached if fresh, otherwise fetch latest.
    Unspecified,
    /// `crgx tool@latest` — always check registry for latest.
    Latest,
    /// `crgx tool@1.2.3` — exact version.
    Exact(semver::Version),
}

impl CrateSpec {
    /// Parse a crate specifier string like `tokei`, `tokei@13.0.0`, or `tokei@latest`.
    pub fn parse(s: &str) -> Result<Self, String> {
        if let Some((name, version_str)) = s.split_once('@') {
            validate_crate_name(name)?;
            let name = name.to_string();
            if version_str.eq_ignore_ascii_case("latest") {
                Ok(CrateSpec {
                    name,
                    version: VersionReq::Latest,
                })
            } else {
                let version = semver::Version::parse(version_str)
                    .map_err(|e| format!("invalid version '{version_str}': {e}"))?;
                Ok(CrateSpec {
                    name,
                    version: VersionReq::Exact(version),
                })
            }
        } else {
            validate_crate_name(s)?;
            Ok(CrateSpec {
                name: s.to_string(),
                version: VersionReq::Unspecified,
            })
        }
    }
}

/// Enforce crates.io's name rules. The name becomes a cache path component,
/// so this also rules out path traversal like `../x`.
fn validate_crate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("crate name cannot be empty".into());
    }
    if name.len() > 64 {
        return Err("crate name is longer than 64 characters".into());
    }
    if !name.starts_with(|c: char| c.is_ascii_alphabetic())
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "invalid crate name '{name}': use letters, digits, '-' and '_', starting with a letter"
        ));
    }
    Ok(())
}

impl fmt::Display for CrateSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.version {
            VersionReq::Unspecified => write!(f, "{}", self.name),
            VersionReq::Latest => write!(f, "{}@latest", self.name),
            VersionReq::Exact(v) => write!(f, "{}@{}", self.name, v),
        }
    }
}

/// Cargo feature selection for building from source.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildOpts {
    pub features: Vec<String>,
    pub no_default_features: bool,
    pub all_features: bool,
}

impl BuildOpts {
    /// Add features from a comma- or space-separated list (as `cargo --features` accepts).
    pub fn add_features(&mut self, list: &str) {
        for f in list.split([',', ' ']).filter(|f| !f.is_empty()) {
            if !self.features.iter().any(|x| x == f) {
                self.features.push(f.to_string());
            }
        }
    }

    /// True when no feature flags were given (pre-built binaries are acceptable).
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Extra arguments for `cargo install`.
    pub fn cargo_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if !self.features.is_empty() {
            args.push("--features".to_string());
            args.push(self.features.join(","));
        }
        if self.no_default_features {
            args.push("--no-default-features".to_string());
        }
        if self.all_features {
            args.push("--all-features".to_string());
        }
        args
    }

    /// Cache namespace for a crate built with these options.
    ///
    /// Binaries built with non-default features must not share a cache slot
    /// with the pre-built binary, so they get e.g. `cargo-about+features=cli`.
    /// `/` and `:` (valid in feature names, not in file names) are mapped to
    /// `~` and `^`, which cargo does not allow in feature names.
    pub fn cache_key(&self, crate_name: &str) -> String {
        if self.is_default() {
            return crate_name.to_string();
        }
        let mut key = crate_name.to_string();
        if self.all_features {
            key.push_str("+all-features");
        }
        if self.no_default_features {
            key.push_str("+no-default-features");
        }
        if !self.features.is_empty() {
            let mut features: Vec<String> = self
                .features
                .iter()
                .map(|f| f.replace('/', "~").replace(':', "^"))
                .collect();
            features.sort();
            key.push_str("+features=");
            key.push_str(&features.join(","));
        }
        key
    }
}

/// Host target triple, captured at compile time.
pub struct TargetTriple;

impl TargetTriple {
    /// Returns the target triple this binary was compiled for.
    pub fn host() -> &'static str {
        env!("CRGX_TARGET")
    }

    /// Target triples whose binaries can run on this host, most preferred first.
    pub fn candidates() -> Vec<String> {
        let host = Self::host();
        let arch = Self::parse(host).arch;
        Self::candidates_for(host, |probe| match probe {
            Probe::Glibc => glibc_available(&arch),
            Probe::Rosetta => rosetta_available(),
        })
    }

    /// Candidate targets for `host`, in preference order (host first).
    ///
    /// Fallbacks mirror what the OS can actually execute: static musl binaries
    /// run on any Linux, glibc binaries only where the glibc loader for this
    /// arch exists, x86_64 macOS binaries only with Rosetta, and so on.
    /// `probe` is only consulted for fallbacks that depend on the system.
    pub fn candidates_for(host: &str, probe: impl Fn(Probe) -> bool) -> Vec<String> {
        let parts = Self::parse(host);
        let mut out = vec![host.to_string()];

        match parts.os.as_str() {
            "linux" => {
                let libc = parts.libc();
                if let Some(rest) = libc.strip_prefix("gnu") {
                    out.push(format!("{}-{}-linux-musl{rest}", parts.arch, parts.vendor));
                } else if let Some(rest) = libc.strip_prefix("musl")
                    && probe(Probe::Glibc)
                {
                    out.push(format!("{}-{}-linux-gnu{rest}", parts.arch, parts.vendor));
                }
            }
            "darwin" => {
                out.push("universal-apple-darwin".to_string());
                out.push("universal2-apple-darwin".to_string());
                if parts.arch == "aarch64" && probe(Probe::Rosetta) {
                    out.push("x86_64-apple-darwin".to_string());
                }
            }
            "windows" => {
                match (parts.libc(), parts.arch.as_str()) {
                    // Rust's GNU-ABI target for ARM64 Windows is `gnullvm`
                    ("msvc", "aarch64") => out.push("aarch64-pc-windows-gnullvm".to_string()),
                    ("msvc", arch) => out.push(format!("{arch}-pc-windows-gnu")),
                    ("gnu" | "gnullvm", arch) => out.push(format!("{arch}-pc-windows-msvc")),
                    _ => {}
                }
                if parts.arch == "aarch64" {
                    // x64 emulation on Windows 11 ARM
                    out.push("x86_64-pc-windows-msvc".to_string());
                }
            }
            _ => {}
        }

        let mut seen = std::collections::HashSet::new();
        out.retain(|t| seen.insert(t.clone()));
        out
    }

    /// Parse target triple into components.
    pub fn parse(triple: &str) -> TargetParts {
        let parts: Vec<&str> = triple.splitn(4, '-').collect();
        match parts.len() {
            4 => TargetParts {
                arch: parts[0].to_string(),
                vendor: parts[1].to_string(),
                os: parts[2].to_string(),
                env: Some(parts[3].to_string()),
            },
            3 => TargetParts {
                arch: parts[0].to_string(),
                vendor: parts[1].to_string(),
                os: parts[2].to_string(),
                env: None,
            },
            _ => TargetParts {
                arch: triple.to_string(),
                vendor: String::new(),
                os: String::new(),
                env: None,
            },
        }
    }
}

/// A system capability that decides whether a fallback target can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    /// A real glibc dynamic loader for the host arch (Linux).
    Glibc,
    /// Rosetta 2, to run x86_64 binaries on Apple Silicon.
    Rosetta,
}

/// glibc's dynamic loader file name for `arch`, if known.
fn glibc_loader(arch: &str) -> Option<&'static str> {
    Some(match arch {
        "x86_64" => "ld-linux-x86-64.so.2",
        "aarch64" => "ld-linux-aarch64.so.1",
        "i586" | "i686" => "ld-linux.so.2",
        "armv7" => "ld-linux-armhf.so.3",
        "riscv64gc" => "ld-linux-riscv64-lp64d.so.1",
        "powerpc64le" => "ld64.so.2",
        "s390x" => "ld64.so.1",
        _ => return None,
    })
}

/// Whether this system can run glibc-linked `arch` binaries.
///
/// Looks for glibc's loader for that arch. Alpine's `gcompat` installs a
/// loader of the same name that only partly emulates glibc, so the file's
/// contents are checked too.
fn glibc_available(arch: &str) -> bool {
    let Some(loader) = glibc_loader(arch) else {
        return false;
    };
    ["/lib64", "/lib", "/usr/lib64", "/usr/lib"]
        .iter()
        .filter_map(|dir| std::fs::read(Path::new(dir).join(loader)).ok())
        .any(|bytes| is_real_glibc_loader(&bytes))
}

/// glibc's loader defines `GLIBC_*` symbol versions; the gcompat shim
/// has none and is itself loaded by musl's `ld-musl-*`.
fn is_real_glibc_loader(bytes: &[u8]) -> bool {
    let has = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
    has(b"GLIBC_") && !has(b"ld-musl")
}

/// Whether Rosetta 2 is installed (macOS on Apple Silicon).
fn rosetta_available() -> bool {
    Path::new("/Library/Apple/usr/libexec/oah/libRosettaRuntime").exists()
}

/// Parsed components of a target triple.
#[derive(Debug, Clone)]
pub struct TargetParts {
    pub arch: String,
    pub vendor: String,
    pub os: String,
    pub env: Option<String>,
}

impl TargetParts {
    /// Returns the libc component (e.g. "gnu", "musl") or empty string.
    pub fn libc(&self) -> &str {
        self.env.as_deref().unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bare_name() {
        let spec = CrateSpec::parse("tokei").unwrap();
        assert_eq!(spec.name, "tokei");
        assert_eq!(spec.version, VersionReq::Unspecified);
    }

    #[test]
    fn parse_exact_version() {
        let spec = CrateSpec::parse("tokei@13.0.0").unwrap();
        assert_eq!(spec.name, "tokei");
        assert!(
            matches!(spec.version, VersionReq::Exact(v) if v == semver::Version::new(13, 0, 0))
        );
    }

    #[test]
    fn parse_latest() {
        let spec = CrateSpec::parse("tokei@latest").unwrap();
        assert_eq!(spec.name, "tokei");
        assert_eq!(spec.version, VersionReq::Latest);
    }

    #[test]
    fn parse_latest_case_insensitive() {
        let spec = CrateSpec::parse("tokei@LATEST").unwrap();
        assert_eq!(spec.version, VersionReq::Latest);
    }

    #[test]
    fn parse_empty_name() {
        assert!(CrateSpec::parse("").is_err());
        assert!(CrateSpec::parse("@1.0.0").is_err());
    }

    #[test]
    fn parse_invalid_version() {
        assert!(CrateSpec::parse("tokei@abc").is_err());
    }

    #[test]
    fn parse_target_triple() {
        let parts = TargetTriple::parse("x86_64-unknown-linux-gnu");
        assert_eq!(parts.arch, "x86_64");
        assert_eq!(parts.vendor, "unknown");
        assert_eq!(parts.os, "linux");
        assert_eq!(parts.env.as_deref(), Some("gnu"));
    }

    #[test]
    fn parse_target_triple_no_env() {
        let parts = TargetTriple::parse("x86_64-apple-darwin");
        assert_eq!(parts.arch, "x86_64");
        assert_eq!(parts.vendor, "apple");
        assert_eq!(parts.os, "darwin");
        assert_eq!(parts.env, None);
    }

    #[test]
    fn build_opts_default() {
        let opts = BuildOpts::default();
        assert!(opts.is_default());
        assert!(opts.cargo_args().is_empty());
        assert_eq!(opts.cache_key("tool"), "tool");
    }

    #[test]
    fn build_opts_add_features_splits_and_dedupes() {
        let mut opts = BuildOpts::default();
        opts.add_features("cli,json");
        opts.add_features("yaml cli");
        opts.add_features(",,");
        assert_eq!(opts.features, ["cli", "json", "yaml"]);
        assert!(!opts.is_default());
    }

    #[test]
    fn build_opts_cargo_args() {
        let opts = BuildOpts {
            features: vec!["cli".into(), "serde/std".into()],
            no_default_features: true,
            all_features: false,
        };
        assert_eq!(
            opts.cargo_args(),
            ["--features", "cli,serde/std", "--no-default-features"]
        );
        let opts = BuildOpts {
            all_features: true,
            ..Default::default()
        };
        assert_eq!(opts.cargo_args(), ["--all-features"]);
    }

    #[test]
    fn build_opts_cache_key_is_order_independent_and_path_safe() {
        let mut a = BuildOpts::default();
        a.add_features("zeta,serde/std,dep:x");
        let mut b = BuildOpts::default();
        b.add_features("dep:x zeta serde/std");
        assert_eq!(a.cache_key("t"), b.cache_key("t"));
        let key = a.cache_key("t");
        assert_eq!(key, "t+features=dep^x,serde~std,zeta");
        assert!(!key.contains(['/', '\\', ':']));
    }

    #[test]
    fn build_opts_cache_key_distinguishes_flags() {
        let feat = BuildOpts {
            features: vec!["cli".into()],
            ..Default::default()
        };
        let nodef = BuildOpts {
            no_default_features: true,
            ..feat.clone()
        };
        let all = BuildOpts {
            all_features: true,
            ..Default::default()
        };
        assert_eq!(feat.cache_key("t"), "t+features=cli");
        assert_eq!(nodef.cache_key("t"), "t+no-default-features+features=cli");
        assert_eq!(all.cache_key("t"), "t+all-features");
    }

    /// Candidates with every probe answered by `yes`.
    fn candidates(host: &str, yes: bool) -> Vec<String> {
        TargetTriple::candidates_for(host, |_| yes)
    }

    #[test]
    fn crate_name_validation() {
        for ok in ["tokei", "cargo-about", "a_b", "x1", "A"] {
            assert!(CrateSpec::parse(ok).is_ok(), "{ok}");
        }
        for bad in [
            "../x", "..", "a/b", "a\\b", "1abc", "-x", "a b", "a.b", "tökei", "C:x",
        ] {
            assert!(CrateSpec::parse(bad).is_err(), "{bad}");
            assert!(
                CrateSpec::parse(&format!("{bad}@1.0.0")).is_err(),
                "{bad}@1.0.0"
            );
        }
        assert!(CrateSpec::parse(&"a".repeat(64)).is_ok());
        assert!(CrateSpec::parse(&"a".repeat(65)).is_err());
    }

    #[test]
    fn glibc_loader_names() {
        assert_eq!(glibc_loader("x86_64"), Some("ld-linux-x86-64.so.2"));
        assert_eq!(glibc_loader("aarch64"), Some("ld-linux-aarch64.so.1"));
        assert_eq!(glibc_loader("mips64"), None);
        // Unknown arch: never assume glibc
        assert!(!glibc_available("mips64"));
    }

    #[test]
    fn gcompat_loader_is_not_glibc() {
        // Shaped like the real files: glibc's ld.so defines GLIBC_* versions,
        // gcompat's shim has an ld-musl interpreter and no GLIBC_* versions.
        assert!(is_real_glibc_loader(
            b"\x7fELF...GLIBC_2.2.5\0GLIBC_PRIVATE\0"
        ));
        assert!(!is_real_glibc_loader(
            b"\x7fELF.../lib/ld-musl-x86_64.so.1\0"
        ));
        assert!(!is_real_glibc_loader(
            b"\x7fELF...ld-musl-x86_64.so.1 GLIBC_2.2.5"
        ));
        assert!(!is_real_glibc_loader(b""));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn glibc_detection_matches_host_libc() {
        // On a gnu build the system necessarily has glibc.
        if TargetTriple::host().ends_with("-linux-gnu") {
            let arch = TargetTriple::parse(TargetTriple::host()).arch;
            assert!(glibc_available(&arch));
        }
    }

    #[test]
    fn probes_only_asked_when_relevant() {
        let asked = std::cell::RefCell::new(Vec::new());
        let probe = |p| {
            asked.borrow_mut().push(p);
            true
        };
        TargetTriple::candidates_for("x86_64-unknown-linux-gnu", probe);
        TargetTriple::candidates_for("x86_64-pc-windows-msvc", probe);
        TargetTriple::candidates_for("x86_64-apple-darwin", probe);
        assert!(asked.borrow().is_empty());
        TargetTriple::candidates_for("x86_64-unknown-linux-musl", probe);
        TargetTriple::candidates_for("aarch64-apple-darwin", probe);
        assert_eq!(*asked.borrow(), [Probe::Glibc, Probe::Rosetta]);
    }

    #[test]
    fn candidates_linux_gnu_falls_back_to_musl() {
        assert_eq!(
            candidates("x86_64-unknown-linux-gnu", true),
            ["x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl"]
        );
        assert_eq!(
            candidates("armv7-unknown-linux-gnueabihf", true),
            [
                "armv7-unknown-linux-gnueabihf",
                "armv7-unknown-linux-musleabihf"
            ]
        );
    }

    #[test]
    fn candidates_linux_musl_uses_gnu_only_with_glibc() {
        assert_eq!(
            candidates("x86_64-unknown-linux-musl", true),
            ["x86_64-unknown-linux-musl", "x86_64-unknown-linux-gnu"]
        );
        // e.g. Alpine: glibc binaries would not run
        assert_eq!(
            candidates("x86_64-unknown-linux-musl", false),
            ["x86_64-unknown-linux-musl"]
        );
    }

    #[test]
    fn candidates_macos() {
        assert_eq!(
            candidates("aarch64-apple-darwin", true),
            [
                "aarch64-apple-darwin",
                "universal-apple-darwin",
                "universal2-apple-darwin",
                "x86_64-apple-darwin"
            ]
        );
        // Without Rosetta, x86_64 binaries can't run
        assert_eq!(
            candidates("aarch64-apple-darwin", false),
            [
                "aarch64-apple-darwin",
                "universal-apple-darwin",
                "universal2-apple-darwin"
            ]
        );
        assert_eq!(
            candidates("x86_64-apple-darwin", false),
            [
                "x86_64-apple-darwin",
                "universal-apple-darwin",
                "universal2-apple-darwin"
            ]
        );
    }

    #[test]
    fn candidates_windows() {
        assert_eq!(
            candidates("x86_64-pc-windows-msvc", false),
            ["x86_64-pc-windows-msvc", "x86_64-pc-windows-gnu"]
        );
        assert_eq!(
            candidates("aarch64-pc-windows-msvc", false),
            [
                "aarch64-pc-windows-msvc",
                "aarch64-pc-windows-gnullvm",
                "x86_64-pc-windows-msvc"
            ]
        );
    }

    #[test]
    fn candidates_host_always_first() {
        let host = TargetTriple::host();
        assert_eq!(TargetTriple::candidates()[0], host);
    }

    #[test]
    fn display_crate_spec() {
        assert_eq!(CrateSpec::parse("tokei").unwrap().to_string(), "tokei");
        assert_eq!(
            CrateSpec::parse("tokei@latest").unwrap().to_string(),
            "tokei@latest"
        );
        assert_eq!(
            CrateSpec::parse("tokei@13.0.0").unwrap().to_string(),
            "tokei@13.0.0"
        );
    }
}
