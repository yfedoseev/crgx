pub mod binstall;
pub mod build;
pub mod github;
pub mod quickinstall;

use crate::config::BuildOpts;
use crate::error::CrgxError;
use crate::registry::CrateInfo;
use crate::verbose::verbose;

/// Result of binary resolution — where to download from.
#[derive(Debug)]
pub struct Resolution {
    /// The download URL.
    pub url: String,
    /// Human-readable source description (e.g. "GitHub Releases", "binstall", "quickinstall").
    pub source: String,
    /// Optional path to the binary within the archive (from binstall bin-dir).
    pub bin_path: Option<String>,
    /// Target triple of the resolved binary (may be a fallback, e.g. musl on a gnu host).
    pub target: String,
}

/// Try all resolution strategies in order: binstall → github → quickinstall → build.
///
/// `targets` lists compatible target triples, most preferred first; each
/// strategy tries all of them before the next strategy is consulted.
pub fn resolve(
    crate_name: &str,
    version: &str,
    targets: &[String],
    bin_name: &str,
    info: &CrateInfo,
    allow_build: bool,
    build_opts: &BuildOpts,
) -> Result<Resolution, CrgxError> {
    // Pre-built binaries can't honor feature selection, so go straight to source.
    if !build_opts.is_default() {
        verbose!("crgx: feature flags given, building from source");
        return build::resolve(crate_name, version, build_opts);
    }

    // 1. Try binstall metadata
    match binstall::resolve(crate_name, version, targets, bin_name, info) {
        Ok(Some(r)) => return Ok(r),
        Ok(None) => {}
        Err(e) => verbose!("crgx: binstall resolution failed: {e}"),
    }

    // 2. Try GitHub Releases
    if let Some(repo_url) = &info.repository {
        match github::resolve(crate_name, version, targets, bin_name, repo_url) {
            Ok(Some(r)) => return Ok(r),
            Ok(None) => {}
            Err(e) => verbose!("crgx: GitHub release resolution failed: {e}"),
        }
    }

    // 3. Try QuickInstall
    match quickinstall::resolve(crate_name, version, targets) {
        Ok(Some(r)) => return Ok(r),
        Ok(None) => {}
        Err(e) => verbose!("crgx: quickinstall resolution failed: {e}"),
    }

    // 4. Cargo build (opt-in)
    if allow_build {
        return build::resolve(crate_name, version, build_opts);
    }

    Err(CrgxError::NoBinaryFound {
        crate_name: crate_name.to_string(),
        version: version.to_string(),
        target: targets.join(", "),
    })
}
