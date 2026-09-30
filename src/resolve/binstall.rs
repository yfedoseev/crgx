use crate::config::TargetTriple;
use crate::error::CrgxError;
use crate::http;
use crate::registry::{self, BinstallMeta, CrateInfo};
use crate::resolve::Resolution;

/// Try to resolve a binary URL using binstall metadata from the crate's Cargo.toml.
pub fn resolve(
    crate_name: &str,
    version: &str,
    targets: &[String],
    bin_name: &str,
    info: &CrateInfo,
) -> Result<Option<Resolution>, CrgxError> {
    let meta = match registry::fetch_cargo_toml(crate_name, version)? {
        Some(m) => m,
        None => return Ok(None),
    };

    let repo = info.repository.as_deref().unwrap_or("");

    for candidate in candidates(&meta, crate_name, version, targets, bin_name, repo) {
        // HEAD request to verify URL exists
        if url_exists(&candidate.url)? {
            return Ok(Some(candidate));
        }
    }

    Ok(None)
}

/// Expand the binstall templates for each target, in preference order.
fn candidates(
    meta: &BinstallMeta,
    crate_name: &str,
    version: &str,
    targets: &[String],
    bin_name: &str,
    repo: &str,
) -> Vec<Resolution> {
    let mut out: Vec<Resolution> = Vec::new();
    for target in targets {
        let Some(pkg_url) = resolve_pkg_url(meta, target) else {
            continue;
        };
        let url = expand_template(&pkg_url, crate_name, version, target, bin_name, repo);
        // A target-independent pkg-url expands identically for every target
        if out.iter().any(|r| r.url == url) {
            continue;
        }
        let bin_path = resolve_bin_dir(meta, target)
            .map(|d| expand_template(&d, crate_name, version, target, bin_name, repo));
        out.push(Resolution {
            url,
            source: "binstall".to_string(),
            bin_path,
            target: target.clone(),
        });
    }
    out
}

fn resolve_pkg_url(meta: &BinstallMeta, target: &str) -> Option<String> {
    // Check exact target match in overrides first
    for ov in &meta.overrides {
        if ov.target == target
            && let Some(url) = &ov.pkg_url
        {
            return Some(url.clone());
        }
    }
    meta.pkg_url.clone()
}

fn resolve_bin_dir(meta: &BinstallMeta, target: &str) -> Option<String> {
    for ov in &meta.overrides {
        if ov.target == target
            && let Some(d) = &ov.bin_dir
        {
            return Some(d.clone());
        }
    }
    meta.bin_dir.clone()
}

/// Expand a binstall template string with all variables.
pub fn expand_template(
    template: &str,
    name: &str,
    version: &str,
    target: &str,
    bin: &str,
    repo: &str,
) -> String {
    let parts = TargetTriple::parse(target);
    let binary_ext = if target.contains("windows") {
        ".exe"
    } else {
        ""
    };
    let archive_suffix = archive_suffix_for(target);
    let archive_format = if archive_suffix == ".zip" {
        "zip"
    } else if archive_suffix == ".tar.xz" {
        "txz"
    } else {
        "tgz"
    };

    // Strip trailing .git from repo URL
    let repo = repo.trim_end_matches(".git");

    template
        .replace("{ name }", name)
        .replace("{name}", name)
        .replace("{ version }", version)
        .replace("{version}", version)
        .replace("{ target }", target)
        .replace("{target}", target)
        .replace("{ bin }", bin)
        .replace("{bin}", bin)
        .replace("{ repo }", repo)
        .replace("{repo}", repo)
        .replace("{ binary-ext }", binary_ext)
        .replace("{binary-ext}", binary_ext)
        .replace("{ archive-suffix }", &archive_suffix)
        .replace("{archive-suffix}", &archive_suffix)
        .replace("{ archive-format }", archive_format)
        .replace("{archive-format}", archive_format)
        .replace("{ target-arch }", &parts.arch)
        .replace("{target-arch}", &parts.arch)
        .replace("{ target-vendor }", &parts.vendor)
        .replace("{target-vendor}", &parts.vendor)
        .replace("{ target-os }", &parts.os)
        .replace("{target-os}", &parts.os)
        .replace("{ target-libc }", parts.libc())
        .replace("{target-libc}", parts.libc())
        .replace("{ target-family }", target_family_for(target))
        .replace("{target-family}", target_family_for(target))
}

fn archive_suffix_for(target: &str) -> String {
    if target.contains("windows") {
        ".zip".to_string()
    } else {
        ".tar.gz".to_string()
    }
}

fn target_family_for(target: &str) -> &str {
    if target.contains("windows") {
        "windows"
    } else {
        "unix"
    }
}

fn url_exists(url: &str) -> Result<bool, CrgxError> {
    let agent = http::agent();
    match agent.head(url).call() {
        Ok(_) => Ok(true),
        Err(ureq::Error::StatusCode(404)) => Ok(false),
        Err(ureq::Error::StatusCode(403)) => Ok(false),
        Err(e) => Err(CrgxError::Network(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::BinstallOverride;

    #[test]
    fn expand_basic_template() {
        let result = expand_template(
            "{ repo }/releases/download/v{ version }/{ name }-{ target }{ archive-suffix }",
            "mytool",
            "1.2.3",
            "x86_64-unknown-linux-gnu",
            "mytool",
            "https://github.com/owner/repo",
        );
        assert_eq!(
            result,
            "https://github.com/owner/repo/releases/download/v1.2.3/mytool-x86_64-unknown-linux-gnu.tar.gz"
        );
    }

    #[test]
    fn expand_template_windows() {
        let result = expand_template(
            "{repo}/releases/download/v{version}/{name}-{target}{archive-suffix}",
            "mytool",
            "1.0.0",
            "x86_64-pc-windows-msvc",
            "mytool",
            "https://github.com/owner/repo",
        );
        assert!(result.ends_with(".zip"));
    }

    #[test]
    fn expand_template_binary_ext() {
        let result = expand_template(
            "{bin}{binary-ext}",
            "mytool",
            "1.0.0",
            "x86_64-unknown-linux-gnu",
            "mytool",
            "",
        );
        assert_eq!(result, "mytool");

        let result = expand_template(
            "{bin}{binary-ext}",
            "mytool",
            "1.0.0",
            "x86_64-pc-windows-msvc",
            "mytool",
            "",
        );
        assert_eq!(result, "mytool.exe");
    }

    #[test]
    fn expand_template_target_parts() {
        let result = expand_template(
            "{target-arch}-{target-os}-{target-libc}",
            "t",
            "1.0.0",
            "x86_64-unknown-linux-gnu",
            "t",
            "",
        );
        assert_eq!(result, "x86_64-linux-gnu");
    }

    fn meta(pkg_url: Option<&str>, overrides: Vec<BinstallOverride>) -> BinstallMeta {
        BinstallMeta {
            pkg_url: pkg_url.map(String::from),
            bin_dir: Some("{ bin }-{ target }/{ bin }{ binary-ext }".to_string()),
            pkg_fmt: None,
            overrides,
        }
    }

    fn targets(ts: &[&str]) -> Vec<String> {
        ts.iter().map(|t| t.to_string()).collect()
    }

    #[test]
    fn candidates_follow_target_order() {
        let m = meta(Some("{ repo }/dl/{ name }-{ target }.tar.gz"), vec![]);
        let c = candidates(
            &m,
            "tool",
            "1.0.0",
            &targets(&["x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl"]),
            "tool",
            "https://github.com/o/r",
        );
        assert_eq!(c.len(), 2);
        assert_eq!(
            c[0].url,
            "https://github.com/o/r/dl/tool-x86_64-unknown-linux-gnu.tar.gz"
        );
        assert_eq!(c[0].target, "x86_64-unknown-linux-gnu");
        assert_eq!(
            c[1].url,
            "https://github.com/o/r/dl/tool-x86_64-unknown-linux-musl.tar.gz"
        );
        assert_eq!(c[1].target, "x86_64-unknown-linux-musl");
        // bin-dir is expanded for the matching target, not the host
        assert_eq!(
            c[1].bin_path.as_deref(),
            Some("tool-x86_64-unknown-linux-musl/tool")
        );
    }

    #[test]
    fn candidates_use_per_target_overrides() {
        let m = meta(
            None,
            vec![BinstallOverride {
                target: "x86_64-unknown-linux-musl".to_string(),
                pkg_url: Some("{ repo }/musl.tgz".to_string()),
                bin_dir: None,
                pkg_fmt: None,
            }],
        );
        let c = candidates(
            &m,
            "tool",
            "1.0.0",
            &targets(&["x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl"]),
            "tool",
            "https://r",
        );
        // gnu has no pkg-url at all; musl comes from its override
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].url, "https://r/musl.tgz");
        assert_eq!(c[0].target, "x86_64-unknown-linux-musl");
    }

    #[test]
    fn candidates_dedupe_target_independent_urls() {
        let m = meta(Some("{ repo }/dl/{ name }.tar.gz"), vec![]);
        let c = candidates(
            &m,
            "tool",
            "1.0.0",
            &targets(&["x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl"]),
            "tool",
            "https://r",
        );
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].target, "x86_64-unknown-linux-gnu");
    }

    #[test]
    fn repo_url_strip_git() {
        let result = expand_template(
            "{repo}/releases",
            "t",
            "1.0.0",
            "x86_64-unknown-linux-gnu",
            "t",
            "https://github.com/owner/repo.git",
        );
        assert_eq!(result, "https://github.com/owner/repo/releases");
    }
}
