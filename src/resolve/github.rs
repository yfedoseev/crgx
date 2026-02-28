use serde::Deserialize;

use crate::error::CrgxError;
use crate::http;
use crate::resolve::Resolution;

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

/// Try to resolve a binary from GitHub Releases.
pub fn resolve(
    crate_name: &str,
    version: &str,
    target: &str,
    _bin_name: &str,
    repo_url: &str,
) -> Result<Option<Resolution>, CrgxError> {
    let (owner, repo) = parse_github_repo(repo_url)?;

    // Try multiple tag formats
    let tag_formats = [
        format!("v{version}"),
        version.to_string(),
        format!("{crate_name}-v{version}"),
        format!("{crate_name}-{version}"),
    ];

    for tag in &tag_formats {
        let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/tags/{tag}");

        let agent = http::agent();
        let mut response = match agent
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json")
            .call()
        {
            Ok(r) => r,
            Err(ureq::Error::StatusCode(404)) => continue,
            Err(e) => return Err(CrgxError::Network(e.to_string())),
        };

        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|e| CrgxError::Network(format!("failed to read GitHub response: {e}")))?;
        let release: GitHubRelease = serde_json::from_str(&body)
            .map_err(|e| CrgxError::Network(format!("failed to parse GitHub response: {e}")))?;

        if let Some(asset) = find_matching_asset(&release.assets, crate_name, version, target) {
            return Ok(Some(Resolution {
                url: asset.browser_download_url.clone(),
                source: format!("GitHub Releases ({}/{}@{})", owner, repo, release.tag_name),
                bin_path: None,
            }));
        }
    }

    Ok(None)
}

/// Parse "owner/repo" from a GitHub URL.
fn parse_github_repo(url: &str) -> Result<(String, String), CrgxError> {
    let url = url.trim_end_matches(".git");

    // Handle https://github.com/owner/repo
    if let Some(rest) = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
    {
        let parts: Vec<&str> = rest.splitn(3, '/').collect();
        if parts.len() >= 2 {
            return Ok((parts[0].to_string(), parts[1].to_string()));
        }
    }

    Err(CrgxError::Other(format!(
        "cannot parse GitHub repo from URL: {url}"
    )))
}

/// Find the best matching asset from a GitHub release.
fn find_matching_asset<'a>(
    assets: &'a [GitHubAsset],
    crate_name: &str,
    version: &str,
    target: &str,
) -> Option<&'a GitHubAsset> {
    // Filter out non-binary assets
    let candidates: Vec<&GitHubAsset> = assets
        .iter()
        .filter(|a| !is_metadata_file(&a.name))
        .filter(|a| !is_package_file(&a.name))
        .collect();

    // Generate expected filename patterns (cargo-binstall's default matrix)
    let patterns = generate_patterns(crate_name, version, target);

    // Try exact matches first
    for pattern in &patterns {
        if let Some(asset) = candidates.iter().find(|a| a.name == *pattern) {
            return Some(asset);
        }
    }

    // Try matching by target triple presence in filename
    for asset in &candidates {
        let name_lower = asset.name.to_lowercase();
        let target_lower = target.to_lowercase();
        if name_lower.contains(&target_lower) && is_archive(&asset.name) {
            return Some(asset);
        }
    }

    // Try matching with underscores instead of hyphens in target
    let target_underscored = target.replace('-', "_");
    for asset in &candidates {
        let name_lower = asset.name.to_lowercase();
        if name_lower.contains(&target_underscored.to_lowercase()) && is_archive(&asset.name) {
            return Some(asset);
        }
    }

    None
}

/// Generate cargo-binstall-style filename patterns.
fn generate_patterns(name: &str, version: &str, target: &str) -> Vec<String> {
    let archive_suffixes = if target.contains("windows") {
        vec![".zip", ".tar.gz"]
    } else {
        vec![".tar.gz", ".tar.xz", ".zip"]
    };

    let mut patterns = Vec::new();

    for suffix in &archive_suffixes {
        // With version
        patterns.push(format!("{name}-{target}-v{version}{suffix}"));
        patterns.push(format!("{name}-{target}-{version}{suffix}"));
        patterns.push(format!("{name}-v{version}-{target}{suffix}"));
        patterns.push(format!("{name}-{version}-{target}{suffix}"));

        // Underscore variants
        patterns.push(format!("{name}_{target}_v{version}{suffix}"));
        patterns.push(format!("{name}_{target}_{version}{suffix}"));

        // Versionless
        patterns.push(format!("{name}-{target}{suffix}"));
        patterns.push(format!("{name}_{target}{suffix}"));
    }

    // Also try raw binary (no archive)
    if target.contains("windows") {
        patterns.push(format!("{name}-{target}-v{version}.exe"));
        patterns.push(format!("{name}-{target}-{version}.exe"));
        patterns.push(format!("{name}.exe"));
    } else {
        patterns.push(format!("{name}-{target}-v{version}"));
        patterns.push(format!("{name}-{target}-{version}"));
    }

    patterns
}

fn is_metadata_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".sha256")
        || lower.ends_with(".sha256sum")
        || lower.ends_with(".sha512")
        || lower.ends_with(".sha512sum")
        || lower.ends_with(".sig")
        || lower.ends_with(".asc")
        || lower.ends_with(".md5")
        || lower.ends_with(".txt")
        || lower.ends_with(".json")
}

fn is_package_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".deb")
        || lower.ends_with(".rpm")
        || lower.ends_with(".msi")
        || lower.ends_with(".pkg")
        || lower.ends_with(".dmg")
        || lower.ends_with(".appimage")
}

fn is_archive(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".tar.gz")
        || lower.ends_with(".tgz")
        || lower.ends_with(".tar.xz")
        || lower.ends_with(".zip")
        || lower.ends_with(".tar.bz2")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_github_url() {
        let (owner, repo) = parse_github_repo("https://github.com/XAMPPRocky/tokei").unwrap();
        assert_eq!(owner, "XAMPPRocky");
        assert_eq!(repo, "tokei");
    }

    #[test]
    fn parse_github_url_with_git() {
        let (owner, repo) = parse_github_repo("https://github.com/owner/repo.git").unwrap();
        assert_eq!(owner, "owner");
        assert_eq!(repo, "repo");
    }

    #[test]
    fn parse_github_url_invalid() {
        assert!(parse_github_repo("https://gitlab.com/owner/repo").is_err());
    }

    #[test]
    fn filter_metadata_files() {
        assert!(is_metadata_file("tool.sha256"));
        assert!(is_metadata_file("checksums.txt"));
        assert!(!is_metadata_file("tool.tar.gz"));
    }

    #[test]
    fn filter_package_files() {
        assert!(is_package_file("tool.deb"));
        assert!(is_package_file("tool.msi"));
        assert!(!is_package_file("tool.tar.gz"));
    }

    #[test]
    fn find_asset_exact_match() {
        let assets = vec![
            GitHubAsset {
                name: "tool-x86_64-unknown-linux-gnu-v1.0.0.tar.gz".to_string(),
                browser_download_url: "https://example.com/tool.tar.gz".to_string(),
            },
            GitHubAsset {
                name: "tool.sha256".to_string(),
                browser_download_url: "https://example.com/tool.sha256".to_string(),
            },
        ];
        let found = find_matching_asset(&assets, "tool", "1.0.0", "x86_64-unknown-linux-gnu");
        assert!(found.is_some());
        assert_eq!(
            found.unwrap().name,
            "tool-x86_64-unknown-linux-gnu-v1.0.0.tar.gz"
        );
    }

    #[test]
    fn find_asset_target_in_name() {
        let assets = vec![GitHubAsset {
            name: "myapp-linux-x86_64-unknown-linux-gnu.tar.gz".to_string(),
            browser_download_url: "https://example.com/myapp.tar.gz".to_string(),
        }];
        let found = find_matching_asset(&assets, "myapp", "1.0.0", "x86_64-unknown-linux-gnu");
        assert!(found.is_some());
    }

    #[test]
    fn generate_patterns_linux() {
        let patterns = generate_patterns("tool", "1.0.0", "x86_64-unknown-linux-gnu");
        assert!(patterns.contains(&"tool-x86_64-unknown-linux-gnu-v1.0.0.tar.gz".to_string()));
        assert!(patterns.contains(&"tool-v1.0.0-x86_64-unknown-linux-gnu.tar.gz".to_string()));
        assert!(patterns.contains(&"tool-x86_64-unknown-linux-gnu.tar.gz".to_string()));
    }
}
