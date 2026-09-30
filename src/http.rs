use std::time::Duration;

use crate::error::CrgxError;
use crate::verbose::verbose;

const USER_AGENT: &str = concat!("crgx/", env!("CARGO_PKG_VERSION"));
const TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_RETRIES: u32 = 2;

/// Upper bound for downloaded archives and `.crate` files. ureq's default
/// (10 MiB) is smaller than many release archives.
pub const MAX_DOWNLOAD_SIZE: u64 = 1 << 30;

/// Create a configured HTTP agent for API calls (30s timeout).
pub fn agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(TIMEOUT))
            .user_agent(USER_AGENT)
            .build(),
    )
}

/// Create a configured HTTP agent for large downloads (5min timeout).
pub fn download_agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(DOWNLOAD_TIMEOUT))
            .user_agent(USER_AGENT)
            .build(),
    )
}

/// Describe the proxy configured in the environment, if any (credentials omitted).
pub fn proxy_description() -> Option<String> {
    ureq::Proxy::try_from_env().map(|p| describe(&p))
}

fn describe(proxy: &ureq::Proxy) -> String {
    let scheme = proxy.protocol().to_string().to_lowercase();
    format!("{scheme}://{}:{}", proxy.host(), proxy.port())
}

/// Execute an HTTP GET with retries for transient errors.
pub fn get_with_retry(agent: &ureq::Agent, url: &str) -> Result<ureq::Body, CrgxError> {
    let mut last_err = None;

    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(500 * u64::from(attempt)));
            verbose!(
                "crgx: retrying download (attempt {}/{MAX_RETRIES})...",
                attempt
            );
        }

        match agent.get(url).call() {
            Ok(response) => return Ok(response.into_body()),
            Err(ureq::Error::StatusCode(code)) => {
                // 5xx = transient, retry. 4xx = permanent, fail immediately.
                if code >= 500 {
                    last_err = Some(CrgxError::HttpStatus {
                        url: url.to_string(),
                        status: code,
                    });
                    continue;
                }
                return Err(CrgxError::HttpStatus {
                    url: url.to_string(),
                    status: code,
                });
            }
            Err(e) => {
                last_err = Some(CrgxError::Network(e.to_string()));
                continue;
            }
        }
    }

    Err(last_err.unwrap_or_else(|| CrgxError::Network("request failed".to_string())))
}

/// Check if an error is a network error (as opposed to a "not found" error).
pub fn is_network_error(err: &CrgxError) -> bool {
    matches!(
        err,
        CrgxError::Network(_)
            | CrgxError::HttpStatus {
                status: 500..=599,
                ..
            }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_proxy_omits_credentials() {
        let p = ureq::Proxy::new("socks5h://user:secret@proxy.local:1080").unwrap();
        assert_eq!(describe(&p), "socks5h://proxy.local:1080");
        let p = ureq::Proxy::new("http://user:secret@proxy.local:3128").unwrap();
        assert_eq!(describe(&p), "http://proxy.local:3128");
    }
}
