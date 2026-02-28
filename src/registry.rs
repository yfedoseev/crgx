use serde::Deserialize;

use crate::config::VersionReq;
use crate::error::CrgxError;

const CRATES_IO_API: &str = "https://crates.io/api/v1";
const USER_AGENT: &str = concat!("crgx/", env!("CARGO_PKG_VERSION"));

/// Crate information from crates.io.
#[derive(Debug)]
pub struct CrateInfo {
    pub name: String,
    pub repository: Option<String>,
    pub versions: Vec<CrateVersion>,
}

/// Version information.
#[derive(Debug)]
pub struct CrateVersion {
    pub num: String,
    pub yanked: bool,
    pub bin_names: Vec<String>,
}

#[derive(Deserialize)]
struct ApiCrateResponse {
    #[serde(rename = "crate")]
    krate: ApiCrate,
    versions: Vec<ApiVersion>,
}

#[derive(Deserialize)]
struct ApiCrate {
    #[allow(dead_code)]
    id: String,
    repository: Option<String>,
}

#[derive(Deserialize)]
struct ApiVersion {
    num: String,
    yanked: bool,
    bin_names: Option<Vec<String>>,
}

/// Fetch crate information from crates.io.
pub fn get_crate(name: &str) -> Result<CrateInfo, CrgxError> {
    let url = format!("{CRATES_IO_API}/crates/{name}");
    let mut response = ureq::get(&url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| match &e {
            ureq::Error::StatusCode(404) => CrgxError::CrateNotFound(name.to_string()),
            ureq::Error::StatusCode(code) => CrgxError::HttpStatus {
                url: url.clone(),
                status: *code,
            },
            _ => CrgxError::Network(e.to_string()),
        })?;

    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|e| CrgxError::Network(format!("failed to read crates.io response: {e}")))?;
    let api_resp: ApiCrateResponse = serde_json::from_str(&body)
        .map_err(|e| CrgxError::Network(format!("failed to parse crates.io response: {e}")))?;

    Ok(CrateInfo {
        name: name.to_string(),
        repository: api_resp.krate.repository,
        versions: api_resp
            .versions
            .into_iter()
            .map(|v| CrateVersion {
                num: v.num,
                yanked: v.yanked,
                bin_names: v.bin_names.unwrap_or_default(),
            })
            .collect(),
    })
}

/// Resolve which version to use based on the version requirement.
pub fn resolve_version(
    info: &CrateInfo,
    req: &VersionReq,
) -> Result<semver::Version, CrgxError> {
    match req {
        VersionReq::Exact(v) => {
            // Verify it exists
            if info.versions.iter().any(|cv| cv.num == v.to_string()) {
                Ok(v.clone())
            } else {
                Err(CrgxError::VersionNotFound {
                    crate_name: info.name.clone(),
                    version: v.to_string(),
                })
            }
        }
        VersionReq::Latest | VersionReq::Unspecified => {
            // Find the latest non-yanked version
            info.versions
                .iter()
                .filter(|v| !v.yanked)
                .filter_map(|v| semver::Version::parse(&v.num).ok())
                .filter(|v| v.pre.is_empty()) // skip pre-releases
                .max()
                .ok_or_else(|| CrgxError::CrateNotFound(info.name.clone()))
        }
    }
}

/// Download and parse the Cargo.toml from a .crate file to extract binstall metadata.
pub fn fetch_cargo_toml(
    name: &str,
    version: &str,
) -> Result<Option<BinstallMeta>, CrgxError> {
    let url = format!(
        "https://static.crates.io/crates/{name}/{name}-{version}.crate"
    );

    let mut response = ureq::get(&url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| match &e {
            ureq::Error::StatusCode(code) => CrgxError::HttpStatus {
                url: url.clone(),
                status: *code,
            },
            _ => CrgxError::Network(e.to_string()),
        })?;

    let body = response
        .body_mut()
        .read_to_vec()
        .map_err(|e| CrgxError::Network(e.to_string()))?;

    // .crate files are .tar.gz
    let gz = flate2::read::GzDecoder::new(std::io::Cursor::new(body));
    let mut archive = tar::Archive::new(gz);

    for entry in archive.entries().map_err(|e| CrgxError::Extraction(e.to_string()))? {
        let mut entry = entry.map_err(|e| CrgxError::Extraction(e.to_string()))?;
        let path = entry
            .path()
            .map_err(|e| CrgxError::Extraction(e.to_string()))?
            .to_path_buf();

        // Look for Cargo.toml (usually at {name}-{version}/Cargo.toml)
        if path.file_name().and_then(|f| f.to_str()) == Some("Cargo.toml")
            && path.components().count() == 2
        {
            let mut contents = String::new();
            std::io::Read::read_to_string(&mut entry, &mut contents)
                .map_err(|e| CrgxError::Extraction(e.to_string()))?;
            return parse_binstall_meta(&contents);
        }
    }

    Ok(None)
}

/// Parsed binstall metadata from Cargo.toml.
#[derive(Debug, Clone)]
pub struct BinstallMeta {
    pub pkg_url: Option<String>,
    pub bin_dir: Option<String>,
    pub pkg_fmt: Option<String>,
    pub overrides: Vec<BinstallOverride>,
}

#[derive(Debug, Clone)]
pub struct BinstallOverride {
    pub target: String,
    pub pkg_url: Option<String>,
    pub bin_dir: Option<String>,
    pub pkg_fmt: Option<String>,
}

fn parse_binstall_meta(cargo_toml: &str) -> Result<Option<BinstallMeta>, CrgxError> {
    let doc: toml::Value =
        toml::from_str(cargo_toml).map_err(|e| CrgxError::Other(format!("bad Cargo.toml: {e}")))?;

    let meta = match doc
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("binstall"))
    {
        Some(m) => m,
        None => return Ok(None),
    };

    let pkg_url = meta.get("pkg-url").and_then(|v| v.as_str()).map(String::from);
    let bin_dir = meta.get("bin-dir").and_then(|v| v.as_str()).map(String::from);
    let pkg_fmt = meta.get("pkg-fmt").and_then(|v| v.as_str()).map(String::from);

    let mut overrides = Vec::new();
    if let Some(overrides_table) = meta.get("overrides").and_then(|v| v.as_table()) {
        for (target, values) in overrides_table {
            overrides.push(BinstallOverride {
                target: target.clone(),
                pkg_url: values.get("pkg-url").and_then(|v| v.as_str()).map(String::from),
                bin_dir: values.get("bin-dir").and_then(|v| v.as_str()).map(String::from),
                pkg_fmt: values.get("pkg-fmt").and_then(|v| v.as_str()).map(String::from),
            });
        }
    }

    Ok(Some(BinstallMeta {
        pkg_url,
        bin_dir,
        pkg_fmt,
        overrides,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_binstall_metadata() {
        let toml = r#"
[package]
name = "test"
version = "1.0.0"

[package.metadata.binstall]
pkg-url = "{ repo }/releases/download/v{ version }/{ name }-{ target }{ archive-suffix }"
bin-dir = "{ bin }{ binary-ext }"
pkg-fmt = "tgz"

[package.metadata.binstall.overrides.x86_64-pc-windows-msvc]
pkg-fmt = "zip"
"#;
        let meta = parse_binstall_meta(toml).unwrap().unwrap();
        assert!(meta.pkg_url.unwrap().contains("{ repo }"));
        assert_eq!(meta.pkg_fmt.as_deref(), Some("tgz"));
        assert_eq!(meta.overrides.len(), 1);
        assert_eq!(meta.overrides[0].target, "x86_64-pc-windows-msvc");
        assert_eq!(meta.overrides[0].pkg_fmt.as_deref(), Some("zip"));
    }

    #[test]
    fn parse_no_binstall_metadata() {
        let toml = r#"
[package]
name = "test"
version = "1.0.0"
"#;
        assert!(parse_binstall_meta(toml).unwrap().is_none());
    }

    #[test]
    fn resolve_latest_version() {
        let info = CrateInfo {
            name: "test".to_string(),
            repository: None,
            versions: vec![
                CrateVersion {
                    num: "1.0.0".to_string(),
                    yanked: false,
                    bin_names: vec![],
                },
                CrateVersion {
                    num: "2.0.0".to_string(),
                    yanked: false,
                    bin_names: vec![],
                },
                CrateVersion {
                    num: "3.0.0-alpha.1".to_string(),
                    yanked: false,
                    bin_names: vec![],
                },
                CrateVersion {
                    num: "2.1.0".to_string(),
                    yanked: true,
                    bin_names: vec![],
                },
            ],
        };
        let v = resolve_version(&info, &VersionReq::Latest).unwrap();
        assert_eq!(v, semver::Version::new(2, 0, 0));
    }

    #[test]
    fn resolve_exact_version() {
        let info = CrateInfo {
            name: "test".to_string(),
            repository: None,
            versions: vec![CrateVersion {
                num: "1.2.3".to_string(),
                yanked: false,
                bin_names: vec![],
            }],
        };
        let v =
            resolve_version(&info, &VersionReq::Exact(semver::Version::new(1, 2, 3))).unwrap();
        assert_eq!(v, semver::Version::new(1, 2, 3));
    }

    #[test]
    fn resolve_missing_version() {
        let info = CrateInfo {
            name: "test".to_string(),
            repository: None,
            versions: vec![],
        };
        assert!(
            resolve_version(&info, &VersionReq::Exact(semver::Version::new(1, 0, 0))).is_err()
        );
    }
}
