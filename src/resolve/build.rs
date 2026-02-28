use std::process::Command;

use crate::error::CrgxError;
use crate::resolve::Resolution;
use crate::verbose::verbose;

/// Build from source using `cargo install` into a temp directory.
pub fn resolve(crate_name: &str, version: &str) -> Result<Resolution, CrgxError> {
    verbose!("crgx: building {crate_name} v{version} from source...");

    let tmp_dir = tempfile::tempdir()
        .map_err(|e| CrgxError::CargoBuild(format!("failed to create temp dir: {e}")))?;

    let status = Command::new("cargo")
        .args([
            "install",
            "--root",
            tmp_dir.path().to_str().unwrap(),
            "--version",
            version,
            crate_name,
        ])
        .status()
        .map_err(|e| CrgxError::CargoBuild(format!("failed to run cargo: {e}")))?;

    if !status.success() {
        return Err(CrgxError::CargoBuild(format!(
            "cargo install exited with status {}",
            status
        )));
    }

    // cargo install puts binaries in {root}/bin/
    // We return a file:// URL pointing to the temp dir so the download layer
    // can handle it. But actually, for cargo build, we handle it specially.
    let bin_dir = tmp_dir.keep().join("bin");

    Ok(Resolution {
        url: format!("file://{}", bin_dir.display()),
        source: "cargo build".to_string(),
        bin_path: None,
    })
}
