//! Base URLs of upstream services.
//!
//! Each can be overridden with an environment variable, which the end-to-end
//! tests use to point crgx at a local mock server.

/// crates.io API (`CRGX_CRATES_IO_API`).
pub fn crates_io_api() -> String {
    env_or("CRGX_CRATES_IO_API", "https://crates.io/api/v1")
}

/// Download host for `.crate` files (`CRGX_CRATES_STATIC`).
pub fn crates_static() -> String {
    env_or("CRGX_CRATES_STATIC", "https://static.crates.io/crates")
}

const GITHUB_API: &str = "https://api.github.com";

/// GitHub REST API (`CRGX_GITHUB_API`).
pub fn github_api() -> String {
    env_or("CRGX_GITHUB_API", GITHUB_API)
}

/// Token for GitHub API requests (`GITHUB_TOKEN`, then `GH_TOKEN`), which
/// raises the rate limit from 60 to 5000 requests per hour.
///
/// Only sent to the real api.github.com, never to an overridden base URL.
pub fn github_token() -> Option<String> {
    token_for(
        &github_api(),
        std::env::var("GITHUB_TOKEN")
            .ok()
            .or_else(|| std::env::var("GH_TOKEN").ok()),
    )
}

fn token_for(api_base: &str, token: Option<String>) -> Option<String> {
    let token = token?.trim().to_string();
    (api_base == GITHUB_API && !token.is_empty()).then_some(token)
}

/// cargo-quickinstall release downloads (`CRGX_QUICKINSTALL_URL`).
pub fn quickinstall() -> String {
    env_or(
        "CRGX_QUICKINSTALL_URL",
        "https://github.com/cargo-bins/cargo-quickinstall/releases/download",
    )
}

fn env_or(var: &str, default: &str) -> String {
    pick(std::env::var(var).ok(), default)
}

/// Use `value` if non-empty (without trailing slashes), else `default`.
fn pick(value: Option<String>, default: &str) -> String {
    value
        .map(|v| v.trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_uses_default_when_unset_or_empty() {
        assert_eq!(pick(None, "https://d"), "https://d");
        assert_eq!(pick(Some(String::new()), "https://d"), "https://d");
        assert_eq!(pick(Some("/".into()), "https://d"), "https://d");
    }

    #[test]
    fn pick_uses_override_without_trailing_slash() {
        assert_eq!(
            pick(Some("http://mock:1/".into()), "https://d"),
            "http://mock:1"
        );
        assert_eq!(
            pick(Some("http://mock:1".into()), "https://d"),
            "http://mock:1"
        );
    }

    #[test]
    fn github_token_only_for_real_api() {
        assert_eq!(
            token_for(GITHUB_API, Some("t0k".into())),
            Some("t0k".to_string())
        );
        assert_eq!(token_for("http://127.0.0.1:9/gh", Some("t0k".into())), None);
        assert_eq!(token_for(GITHUB_API, Some("  ".into())), None);
        assert_eq!(token_for(GITHUB_API, None), None);
    }
}
