use std::fmt;

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
            let name = name.to_string();
            if name.is_empty() {
                return Err("crate name cannot be empty".into());
            }
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
            if s.is_empty() {
                return Err("crate name cannot be empty".into());
            }
            Ok(CrateSpec {
                name: s.to_string(),
                version: VersionReq::Unspecified,
            })
        }
    }
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

/// Host target triple, captured at compile time.
pub struct TargetTriple;

impl TargetTriple {
    /// Returns the target triple this binary was compiled for.
    pub fn host() -> &'static str {
        env!("CRGX_TARGET")
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

    /// Returns the binary extension for the host platform.
    pub fn binary_ext() -> &'static str {
        if cfg!(windows) { ".exe" } else { "" }
    }

    /// Returns the typical archive suffix for the host platform.
    pub fn archive_suffix() -> &'static str {
        if cfg!(windows) {
            ".zip"
        } else {
            ".tar.gz"
        }
    }

    /// Returns the OS family.
    pub fn target_family() -> &'static str {
        if cfg!(unix) {
            "unix"
        } else if cfg!(windows) {
            "windows"
        } else {
            "unknown"
        }
    }
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
        assert!(matches!(spec.version, VersionReq::Exact(v) if v == semver::Version::new(13, 0, 0)));
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
