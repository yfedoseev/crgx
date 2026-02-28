use crate::error::CrgxError;
use crate::resolve::Resolution;

const USER_AGENT: &str = concat!("crgx/", env!("CARGO_PKG_VERSION"));

/// Try to resolve a binary from cargo-quickinstall.
pub fn resolve(
    crate_name: &str,
    version: &str,
    target: &str,
) -> Result<Option<Resolution>, CrgxError> {
    let url = format!(
        "https://github.com/cargo-bins/cargo-quickinstall/releases/download/{crate_name}-{version}/{crate_name}-{version}-{target}.tar.gz"
    );

    // HEAD request to check if it exists
    match ureq::head(&url)
        .header("User-Agent", USER_AGENT)
        .call()
    {
        Ok(_) => Ok(Some(Resolution {
            url,
            source: "cargo-quickinstall".to_string(),
            bin_path: None,
        })),
        Err(ureq::Error::StatusCode(404)) => Ok(None),
        Err(ureq::Error::StatusCode(403)) => Ok(None),
        Err(e) => Err(CrgxError::Network(e.to_string())),
    }
}
