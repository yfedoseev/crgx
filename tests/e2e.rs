//! End-to-end tests: run the real `crgx` binary against a local mock of
//! crates.io, GitHub Releases and cargo-quickinstall (redirected with the
//! `CRGX_*` URL overrides), optionally through real SOCKS5 / HTTP proxies.
//!
//! The "tool" served in release archives is the crgx binary itself, so
//! `tool --version` prints `crgx <version>` on every platform.

mod support;

use predicates::prelude::*;
use support::*;

const VERSION_LINE: &str = concat!("crgx ", env!("CARGO_PKG_VERSION"));

// ---------------------------------------------------------------------------
// #14: fall back to compatible targets (e.g. musl on a glibc host)
// ---------------------------------------------------------------------------

#[test]
fn github_release_falls_back_to_compatible_target() {
    let Some(fallback) = fallback_target() else {
        return eprintln!("skipped: no fallback target for {}", host_target());
    };
    let mock = MockServer::start();
    publish_crate(&mock, "tool", "1.0.0", Some(GH_REPO), "");
    // Mirrors cargo-about 0.9.2: no asset for the host triple, only the fallback.
    publish_release(
        &mock,
        "tool",
        "v1.0.0",
        &[
            (
                &format!("tool-1.0.0-{fallback}.tar.gz"),
                tool_archive("tool"),
            ),
            (
                &format!("tool-1.0.0-{fallback}.tar.gz.sha256"),
                b"x".to_vec(),
            ),
        ],
    );
    let cache = tempfile::tempdir().unwrap();

    crgx(&mock, cache.path())
        .args(["-v", "tool@1.0.0", "--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains(VERSION_LINE))
        .stderr(predicate::str::contains(format!(
            "no {} binary available, using compatible {fallback} binary",
            host_target()
        )));

    let meta = cache_metadata(cache.path(), "tool", "1.0.0");
    assert_eq!(meta["target"], fallback.as_str());
    assert!(
        meta["source"]
            .as_str()
            .unwrap()
            .starts_with("GitHub Releases")
    );
}

#[test]
fn host_target_is_preferred_over_fallback() {
    let Some(fallback) = fallback_target() else {
        return eprintln!("skipped: no fallback target for {}", host_target());
    };
    let host = host_target();
    let mock = MockServer::start();
    publish_crate(&mock, "tool", "1.0.0", Some(GH_REPO), "");
    publish_release(
        &mock,
        "tool",
        "v1.0.0",
        &[
            (
                &format!("tool-1.0.0-{fallback}.tar.gz"),
                tool_archive("tool"),
            ),
            (&format!("tool-1.0.0-{host}.tar.gz"), tool_archive("tool")),
        ],
    );
    let cache = tempfile::tempdir().unwrap();

    crgx(&mock, cache.path())
        .args(["-v", "tool@1.0.0", "--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains(VERSION_LINE))
        .stderr(predicate::str::contains("using compatible").not());

    let downloads = mock.requests_matching("/dl/");
    assert_eq!(downloads, [format!("GET /dl/tool-1.0.0-{host}.tar.gz")]);
    assert_eq!(
        cache_metadata(cache.path(), "tool", "1.0.0")["target"],
        host
    );
}

#[test]
fn binstall_metadata_falls_back_to_compatible_target() {
    let Some(fallback) = fallback_target() else {
        return eprintln!("skipped: no fallback target for {}", host_target());
    };
    let mock = MockServer::start();
    let binstall = format!(
        "[package.metadata.binstall]\n\
         pkg-url = \"{}/pkgs/{{ name }}-{{ target }}.tar.gz\"\n\
         bin-dir = \"{{ name }}-{{ target }}/{{ bin }}{{ binary-ext }}\"\n\
         pkg-fmt = \"tgz\"\n",
        mock.url()
    );
    publish_crate(&mock, "tool", "1.0.0", None, &binstall);
    mock.route(
        &format!("/pkgs/tool-{fallback}.tar.gz"),
        tool_archive_at(&format!("tool-{fallback}/tool")),
    );
    let cache = tempfile::tempdir().unwrap();

    crgx(&mock, cache.path())
        .args(["tool@1.0.0", "--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains(VERSION_LINE));

    // The host URL was probed first and missed.
    let heads = mock.requests_matching("HEAD /pkgs/");
    assert_eq!(
        heads[0],
        format!("HEAD /pkgs/tool-{}.tar.gz", host_target())
    );
    let meta = cache_metadata(cache.path(), "tool", "1.0.0");
    assert_eq!(meta["source"], "binstall");
    assert_eq!(meta["target"], fallback.as_str());
}

#[test]
fn quickinstall_falls_back_to_compatible_target() {
    let Some(fallback) = fallback_target() else {
        return eprintln!("skipped: no fallback target for {}", host_target());
    };
    let mock = MockServer::start();
    publish_crate(&mock, "tool", "1.0.0", None, "");
    mock.route(
        &format!("/qi/tool-1.0.0/tool-1.0.0-{fallback}.tar.gz"),
        tool_archive("tool"),
    );
    let cache = tempfile::tempdir().unwrap();

    crgx(&mock, cache.path())
        .args(["tool@1.0.0", "--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains(VERSION_LINE));

    let meta = cache_metadata(cache.path(), "tool", "1.0.0");
    assert_eq!(meta["source"], "cargo-quickinstall");
    assert_eq!(meta["target"], fallback.as_str());
}

#[test]
fn no_binary_error_lists_every_target_tried() {
    let mock = MockServer::start();
    publish_crate(&mock, "tool", "1.0.0", Some(GH_REPO), "");
    publish_release(
        &mock,
        "tool",
        "v1.0.0",
        &[("tool-1.0.0-sparc-sun-solaris.tar.gz", vec![])],
    );
    let cache = tempfile::tempdir().unwrap();

    let output = crgx(&mock, cache.path())
        .args(["tool@1.0.0"])
        .assert()
        .code(1)
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8(output).unwrap();

    // The full list depends on the system (e.g. Rosetta on macOS), so check
    // its shape: host first, then the first fallback, all in one list.
    let prefix = "no pre-built binary found for tool v1.0.0 (";
    let start = stderr.find(prefix).expect(&stderr) + prefix.len();
    let end = start + stderr[start..].find(')').expect(&stderr);
    let tried: Vec<&str> = stderr[start..end].split(", ").collect();
    assert_eq!(tried[0], host_target(), "{stderr}");
    match fallback_target() {
        Some(fallback) => assert_eq!(tried.get(1), Some(&fallback.as_str()), "{stderr}"),
        None => assert_eq!(tried.len(), 1, "{stderr}"),
    }
}

#[test]
fn musl_host_without_glibc_never_uses_gnu_binaries() {
    if !host_target().contains("-linux-musl") || has_glibc() {
        return eprintln!("skipped: only meaningful on a musl system without glibc (Alpine)");
    }
    let gnu = host_target().replacen("-linux-musl", "-linux-gnu", 1);
    let mock = MockServer::start();
    publish_crate(&mock, "tool", "1.0.0", Some(GH_REPO), "");
    publish_release(
        &mock,
        "tool",
        "v1.0.0",
        &[(&format!("tool-1.0.0-{gnu}.tar.gz"), tool_archive("tool"))],
    );
    let cache = tempfile::tempdir().unwrap();

    crgx(&mock, cache.path())
        .args(["tool@1.0.0"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(format!(
            "no pre-built binary found for tool v1.0.0 ({})",
            host_target()
        )));
    assert!(mock.requests_matching("/dl/").is_empty());
}

#[test]
fn invalid_crate_name_is_rejected_before_any_io() {
    let mock = MockServer::start();
    let cache = tempfile::tempdir().unwrap();
    for spec in ["../../etc@1.0.0", "a/b", "..\\x@1.0.0"] {
        crgx(&mock, cache.path())
            .args([spec])
            .assert()
            .code(2)
            .stderr(predicate::str::contains("invalid crate name"));
    }
    assert!(mock.requests().is_empty());
    assert!(!cache.path().join("bin").exists());
}

// ---------------------------------------------------------------------------
// #13: HTTP(S) and SOCKS proxies from the environment
// ---------------------------------------------------------------------------

/// Serve a crate whose every URL uses a hostname only the proxy can resolve.
fn proxied_fixture() -> (MockServer, ProxyServer) {
    let mock = MockServer::start_with_host(PROXY_ONLY_HOST);
    let proxy = ProxyServer::start(PROXY_ONLY_HOST, mock.addr());
    publish_crate(&mock, "tool", "1.0.0", Some(GH_REPO), "");
    publish_release(
        &mock,
        "tool",
        "v1.0.0",
        &[(
            &format!("tool-1.0.0-{}.tar.gz", host_target()),
            tool_archive("tool"),
        )],
    );
    (mock, proxy)
}

#[test]
fn socks5h_proxy_from_environment() {
    let (mock, proxy) = proxied_fixture();
    let cache = tempfile::tempdir().unwrap();
    let proxy_url = format!("socks5h://127.0.0.1:{}", proxy.port());

    crgx(&mock, cache.path())
        .env("ALL_PROXY", &proxy_url)
        .args(["-v", "tool@1.0.0", "--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains(VERSION_LINE))
        .stderr(predicate::str::contains(format!("using proxy {proxy_url}")));

    // socks5h: the hostname (not a locally resolved IP) went to the proxy,
    // for every request: registry, .crate, GitHub API and the download.
    let log = proxy.log();
    assert!(!log.is_empty());
    assert!(
        log.iter()
            .all(|l| l.starts_with(&format!("socks5 {PROXY_ONLY_HOST}:"))),
        "{log:?}"
    );
    assert!(mock.requests_matching("/dl/").len() == 1);
}

#[test]
fn socks5_proxy_lowercase_env_var() {
    let (mock, proxy) = proxied_fixture();
    let cache = tempfile::tempdir().unwrap();

    crgx(&mock, cache.path())
        .env("all_proxy", format!("socks5h://127.0.0.1:{}", proxy.port()))
        .args(["tool@1.0.0", "--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains(VERSION_LINE));
    assert!(!proxy.log().is_empty());
}

#[test]
fn http_proxy_from_environment() {
    let (mock, proxy) = proxied_fixture();
    let cache = tempfile::tempdir().unwrap();
    let proxy_url = format!("http://127.0.0.1:{}", proxy.port());

    crgx(&mock, cache.path())
        .env("HTTP_PROXY", &proxy_url)
        .args(["-v", "tool@1.0.0", "--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains(VERSION_LINE))
        .stderr(predicate::str::contains(format!("using proxy {proxy_url}")));

    let log = proxy.log();
    assert!(!log.is_empty());
    assert!(log.iter().all(|l| l.contains(PROXY_ONLY_HOST)), "{log:?}");
}

#[test]
fn no_proxy_bypasses_proxy() {
    let mock = MockServer::start();
    publish_crate(&mock, "tool", "1.0.0", Some(GH_REPO), "");
    publish_release(
        &mock,
        "tool",
        "v1.0.0",
        &[(
            &format!("tool-1.0.0-{}.tar.gz", host_target()),
            tool_archive("tool"),
        )],
    );
    let cache = tempfile::tempdir().unwrap();

    crgx(&mock, cache.path())
        .env(
            "ALL_PROXY",
            format!("socks5h://127.0.0.1:{}", closed_port()),
        )
        .env("NO_PROXY", "127.0.0.1")
        .args(["tool@1.0.0", "--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains(VERSION_LINE));
}

#[test]
fn unreachable_proxy_is_not_silently_bypassed() {
    let mock = MockServer::start();
    publish_crate(&mock, "tool", "1.0.0", Some(GH_REPO), "");
    let cache = tempfile::tempdir().unwrap();

    crgx(&mock, cache.path())
        .env(
            "ALL_PROXY",
            format!("socks5h://127.0.0.1:{}", closed_port()),
        )
        .args(["tool@1.0.0"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("network error"));
    assert!(mock.requests().is_empty(), "request bypassed the proxy");
}

// ---------------------------------------------------------------------------
// #14: --features / --no-default-features / --all-features (stub cargo)
// ---------------------------------------------------------------------------

#[cfg(unix)]
mod features {
    use super::*;

    /// A crate with a pre-built binary available, so tests can tell a
    /// source build apart from a download.
    fn fixture() -> MockServer {
        let mock = MockServer::start();
        publish_crate(&mock, "tool", "1.0.0", Some(GH_REPO), "");
        publish_release(
            &mock,
            "tool",
            "v1.0.0",
            &[(
                &format!("tool-1.0.0-{}.tar.gz", host_target()),
                tool_archive("tool"),
            )],
        );
        mock
    }

    #[test]
    fn features_build_from_source_and_skip_prebuilt() {
        let mock = fixture();
        let cache = tempfile::tempdir().unwrap();
        let cargo = StubCargo::new();

        cargo
            .apply(crgx(&mock, cache.path()))
            .args(["-F", "cli", "--features=json,yaml", "tool@1.0.0", "a", "b"])
            .assert()
            .success()
            .stdout("built-from-source a b\n");

        assert_eq!(
            cargo.calls(),
            ["install --root <tmp> --version 1.0.0 tool --features cli,json,yaml"]
        );
        assert!(
            mock.requests_matching("/dl/").is_empty(),
            "pre-built was downloaded"
        );
        let meta = cache_metadata(cache.path(), "tool+features=cli,json,yaml", "1.0.0");
        assert_eq!(meta["source"], "cargo build");
    }

    #[test]
    fn feature_build_is_cached_separately_from_prebuilt() {
        let mock = fixture();
        let cache = tempfile::tempdir().unwrap();
        let cargo = StubCargo::new();

        // Source build with a feature.
        cargo
            .apply(crgx(&mock, cache.path()))
            .args(["--features", "cli", "tool@1.0.0"])
            .assert()
            .success()
            .stdout("built-from-source\n");

        // Same features again: cache hit, cargo not re-run.
        cargo
            .apply(crgx(&mock, cache.path()))
            .args(["--features", "cli", "tool@1.0.0"])
            .assert()
            .success()
            .stdout("built-from-source\n");
        assert_eq!(cargo.calls().len(), 1);

        // No features: the pre-built binary, not the feature build.
        cargo
            .apply(crgx(&mock, cache.path()))
            .args(["tool@1.0.0", "--version"])
            .assert()
            .success()
            .stdout(predicate::str::contains(VERSION_LINE));
        assert_eq!(cargo.calls().len(), 1);

        crgx(&mock, cache.path())
            .arg("--cache-list")
            .assert()
            .success()
            .stdout(predicate::str::contains("tool v1.0.0"))
            .stdout(predicate::str::contains(
                "tool+features=cli v1.0.0 (cargo build)",
            ));
    }

    #[test]
    fn feature_toggles_are_passed_to_cargo() {
        let mock = fixture();
        let cache = tempfile::tempdir().unwrap();
        let cargo = StubCargo::new();

        cargo
            .apply(crgx(&mock, cache.path()))
            .args(["--no-default-features", "--all-features", "tool@1.0.0"])
            .assert()
            .success();

        assert_eq!(
            cargo.calls(),
            ["install --root <tmp> --version 1.0.0 tool --no-default-features --all-features"]
        );
        cache_metadata(
            cache.path(),
            "tool+all-features+no-default-features",
            "1.0.0",
        );
    }

    #[test]
    fn allow_build_without_features_passes_no_feature_args() {
        let mock = MockServer::start();
        publish_crate(&mock, "tool", "1.0.0", None, "");
        let cache = tempfile::tempdir().unwrap();
        let cargo = StubCargo::new();

        cargo
            .apply(crgx(&mock, cache.path()))
            .args(["--allow-build", "tool@1.0.0"])
            .assert()
            .success()
            .stdout("built-from-source\n");
        assert_eq!(cargo.calls(), ["install --root <tmp> --version 1.0.0 tool"]);
        // Without features, the normal cache slot is used.
        cache_metadata(cache.path(), "tool", "1.0.0");
    }

    #[test]
    fn offline_feature_build_not_cached() {
        let mock = fixture();
        let cache = tempfile::tempdir().unwrap();

        // Pre-built binary is cached, but that must not satisfy a feature request.
        crgx(&mock, cache.path())
            .args(["tool@1.0.0", "--version"])
            .assert()
            .success();
        crgx(&mock, cache.path())
            .args(["--offline", "-F", "cli", "tool@1.0.0"])
            .assert()
            .code(1)
            .stderr(predicate::str::contains("is not cached"));
    }
}

// ---------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------

#[test]
fn cache_dir_override() {
    let mock = MockServer::start();
    let cache = tempfile::tempdir().unwrap();
    crgx(&mock, cache.path())
        .arg("--cache-dir")
        .assert()
        .success()
        .stdout(format!("{}\n", cache.path().display()));
}

#[test]
fn help_documents_new_flags() {
    let mock = MockServer::start();
    let cache = tempfile::tempdir().unwrap();
    crgx(&mock, cache.path())
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--features"))
        .stdout(predicate::str::contains("--no-default-features"))
        .stdout(predicate::str::contains("HTTPS_PROXY"))
        .stdout(predicate::str::contains("socks5h://"));
}

#[test]
fn features_flag_requires_value() {
    let mock = MockServer::start();
    let cache = tempfile::tempdir().unwrap();
    crgx(&mock, cache.path())
        .arg("--features")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--features requires a value"));
}

#[test]
fn cached_binary_runs_without_network() {
    let mock = MockServer::start();
    publish_crate(&mock, "tool", "1.0.0", Some(GH_REPO), "");
    publish_release(
        &mock,
        "tool",
        "v1.0.0",
        &[(
            &format!("tool-1.0.0-{}.tar.gz", host_target()),
            tool_archive("tool"),
        )],
    );
    let cache = tempfile::tempdir().unwrap();
    crgx(&mock, cache.path())
        .args(["tool@1.0.0", "--version"])
        .assert()
        .success();
    let before = mock.requests().len();
    crgx(&mock, cache.path())
        .args(["--offline", "tool@1.0.0", "--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains(VERSION_LINE));
    assert_eq!(mock.requests().len(), before);
}

// ---------------------------------------------------------------------------
// Against the real internet (run with `cargo test -- --ignored`)
// ---------------------------------------------------------------------------

/// The exact scenario from issue #14: cargo-about ships musl-only Linux builds.
#[test]
#[ignore = "network"]
fn real_cargo_about_resolves_on_linux() {
    if !host_target().contains("-linux-") {
        return;
    }
    let cache = tempfile::tempdir().unwrap();
    real_crgx(cache.path())
        .args(["-v", "cargo-about@0.9.2", "--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains("cargo-about 0.9.2"));
}

#[test]
fn tar_xz_release_asset() {
    let mock = MockServer::start();
    publish_crate(&mock, "tool", "1.0.0", Some(GH_REPO), "");
    publish_release(
        &mock,
        "tool",
        "v1.0.0",
        &[(
            &format!("tool-1.0.0-{}.tar.xz", host_target()),
            tool_archive_xz("tool"),
        )],
    );
    let cache = tempfile::tempdir().unwrap();
    crgx(&mock, cache.path())
        .args(["tool@1.0.0", "--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains(VERSION_LINE));
}

/// A plain pre-built download on every platform.
#[test]
#[ignore = "network"]
fn real_prebuilt_download() {
    let cache = tempfile::tempdir().unwrap();
    real_crgx(cache.path())
        .args(["-v", "ripgrep@14.1.0", "--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ripgrep 14.1.0"));
}

/// Real crates.io + GitHub over TLS through a SOCKS5h proxy (#13).
#[test]
#[ignore = "network"]
fn real_socks5h_proxy() {
    let proxy = ProxyServer::passthrough();
    let cache = tempfile::tempdir().unwrap();
    real_crgx(cache.path())
        .env("ALL_PROXY", format!("socks5h://127.0.0.1:{}", proxy.port()))
        .args(["-v", "ripgrep@14.1.0", "--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ripgrep 14.1.0"));
    let log = proxy.log();
    assert!(log.contains(&"socks5 crates.io:443".to_string()), "{log:?}");
    assert!(log.iter().any(|l| l.contains("github")), "{log:?}");
}

/// Real crates.io + GitHub over TLS through an HTTP CONNECT proxy (#13).
#[test]
#[ignore = "network"]
fn real_http_connect_proxy() {
    let proxy = ProxyServer::passthrough();
    let cache = tempfile::tempdir().unwrap();
    real_crgx(cache.path())
        .env("HTTPS_PROXY", format!("http://127.0.0.1:{}", proxy.port()))
        .args(["-v", "ripgrep@14.1.0", "--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ripgrep 14.1.0"));
    let log = proxy.log();
    assert!(
        log.contains(&"connect crates.io:443".to_string()),
        "{log:?}"
    );
}

/// crgx against the real upstreams, with an isolated cache and no ambient proxy.
fn real_crgx(cache: &std::path::Path) -> assert_cmd::Command {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("crgx");
    cmd.env("CRGX_CACHE_DIR", cache)
        .timeout(std::time::Duration::from_secs(300));
    for var in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "NO_PROXY"] {
        cmd.env_remove(var).env_remove(var.to_lowercase());
    }
    cmd
}
