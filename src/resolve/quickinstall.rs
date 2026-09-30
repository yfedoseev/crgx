use crate::error::CrgxError;
use crate::http;
use crate::resolve::Resolution;
use crate::upstream;

/// Try to resolve a binary from cargo-quickinstall.
pub fn resolve(
    crate_name: &str,
    version: &str,
    targets: &[String],
) -> Result<Option<Resolution>, CrgxError> {
    let agent = http::agent();
    for target in targets {
        let url = url_for(&upstream::quickinstall(), crate_name, version, target);
        match agent.head(&url).call() {
            Ok(_) => {
                return Ok(Some(Resolution {
                    url,
                    source: "cargo-quickinstall".to_string(),
                    bin_path: None,
                    target: target.clone(),
                }));
            }
            Err(ureq::Error::StatusCode(404)) | Err(ureq::Error::StatusCode(403)) => {}
            Err(e) => return Err(CrgxError::Network(e.to_string())),
        }
    }
    Ok(None)
}

fn url_for(base: &str, crate_name: &str, version: &str, target: &str) -> String {
    format!("{base}/{crate_name}-{version}/{crate_name}-{version}-{target}.tar.gz")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quickinstall_url_layout() {
        assert_eq!(
            url_for(
                "https://github.com/cargo-bins/cargo-quickinstall/releases/download",
                "tokei",
                "13.0.0",
                "x86_64-unknown-linux-musl"
            ),
            "https://github.com/cargo-bins/cargo-quickinstall/releases/download/tokei-13.0.0/tokei-13.0.0-x86_64-unknown-linux-musl.tar.gz"
        );
    }
}
