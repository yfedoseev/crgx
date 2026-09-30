mod cache;
mod cli;
mod config;
mod download;
mod error;
mod exec;
mod http;
mod registry;
mod resolve;
mod upstream;
mod verbose;

use std::process;

fn main() {
    let args = std::env::args().skip(1); // skip argv[0]
    let command = match cli::parse_args(args) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("crgx: {e}");
            process::exit(2);
        }
    };

    match run(command) {
        Ok(code) => process::exit(code),
        Err(e) => {
            eprintln!("crgx: {e}");
            process::exit(1);
        }
    }
}

fn run(command: cli::Command) -> Result<i32, error::CrgxError> {
    match command {
        cli::Command::Help => {
            println!("{}", cli::help_text());
            Ok(0)
        }
        cli::Command::Version => {
            println!("crgx {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        cli::Command::CacheDir => {
            let cache = cache::Cache::open()?;
            println!("{}", cache.dir().display());
            Ok(0)
        }
        cli::Command::CacheList => {
            let cache = cache::Cache::open()?;
            let entries = cache.list()?;
            if entries.is_empty() {
                println!("No cached binaries.");
            } else {
                for entry in entries {
                    println!(
                        "  {} v{} ({})",
                        entry.crate_name, entry.version, entry.source
                    );
                }
            }
            Ok(0)
        }
        cli::Command::CacheClean => {
            let cache = cache::Cache::open()?;
            let count = cache.clean()?;
            println!("Removed {count} cached entries.");
            Ok(0)
        }
        cli::Command::Run(args) => run_crate(args),
    }
}

fn run_crate(args: cli::RunArgs) -> Result<i32, error::CrgxError> {
    use verbose::verbose;

    if args.verbose {
        verbose::enable();
    }

    let cache = cache::Cache::open()?;
    // Builds with non-default features live in their own cache namespace.
    let cache_name = args.build.cache_key(&args.spec.name);

    if !args.offline
        && let Some(proxy) = http::proxy_description()
    {
        verbose!("crgx: using proxy {proxy}");
    }

    // Determine the version to use
    let resolved_version = match &args.spec.version {
        config::VersionReq::Exact(v) => {
            // Exact version — check cache, use if present
            if let Some(entry) = cache.lookup(&cache_name, &v.to_string())? {
                let bin_name = args.bin.as_deref().unwrap_or(&entry.bin_name);
                let bin_path = cache.bin_path(&cache_name, &v.to_string(), bin_name);
                return exec::exec(&bin_path, &args.tool_args);
            }
            if args.offline {
                return Err(error::CrgxError::NotCached(args.spec.to_string()));
            }
            v.clone()
        }
        config::VersionReq::Latest => {
            // Always check registry for latest
            if args.offline {
                // With --offline, just use whatever is cached
                if let Some(entry) = cache.find_latest_cached(&cache_name)? {
                    let bin_name = args.bin.as_deref().unwrap_or(&entry.bin_name);
                    let bin_path = cache.bin_path(&cache_name, &entry.version, bin_name);
                    return exec::exec(&bin_path, &args.tool_args);
                }
                return Err(error::CrgxError::NotCached(args.spec.to_string()));
            }
            verbose!(
                "crgx: checking crates.io for latest version of {}...",
                args.spec.name
            );
            let info = match registry::get_crate(&args.spec.name) {
                Ok(info) => info,
                Err(e) if http::is_network_error(&e) => {
                    // Network failure — fall back to cached version if available
                    if let Some(entry) = cache.find_latest_cached(&cache_name)? {
                        verbose!(
                            "crgx: network unavailable, using cached {} v{}",
                            args.spec.name,
                            entry.version
                        );
                        let bin_name = args.bin.as_deref().unwrap_or(&entry.bin_name);
                        let bin_path = cache.bin_path(&cache_name, &entry.version, bin_name);
                        return exec::exec(&bin_path, &args.tool_args);
                    }
                    return Err(e);
                }
                Err(e) => return Err(e),
            };
            let version = registry::resolve_version(&info, &args.spec.version)?;
            // Check if we already have this version cached
            if let Some(entry) = cache.lookup(&cache_name, &version.to_string())? {
                cache.touch_checked(&cache_name, &version.to_string())?;
                let bin_name = args.bin.as_deref().unwrap_or(&entry.bin_name);
                let bin_path = cache.bin_path(&cache_name, &version.to_string(), bin_name);
                return exec::exec(&bin_path, &args.tool_args);
            }
            version
        }
        config::VersionReq::Unspecified => {
            // Use cached if fresh, otherwise check registry
            let stale_entry = if let Some(entry) = cache.find_latest_cached(&cache_name)? {
                if !cache.is_stale(&cache_name, &entry.version)? {
                    let bin_name = args.bin.as_deref().unwrap_or(&entry.bin_name);
                    let bin_path = cache.bin_path(&cache_name, &entry.version, bin_name);
                    return exec::exec(&bin_path, &args.tool_args);
                }
                // Stale — check for updates
                verbose!("crgx: checking for updates to {}...", args.spec.name);
                Some(entry)
            } else {
                None
            };
            if args.offline {
                if let Some(entry) = stale_entry {
                    let bin_name = args.bin.as_deref().unwrap_or(&entry.bin_name);
                    let bin_path = cache.bin_path(&cache_name, &entry.version, bin_name);
                    return exec::exec(&bin_path, &args.tool_args);
                }
                return Err(error::CrgxError::NotCached(args.spec.to_string()));
            }
            let info = match registry::get_crate(&args.spec.name) {
                Ok(info) => info,
                Err(e) if http::is_network_error(&e) => {
                    // Network failure — fall back to stale cached version if available
                    if let Some(entry) = stale_entry {
                        verbose!(
                            "crgx: network unavailable, using cached {} v{}",
                            args.spec.name,
                            entry.version
                        );
                        cache.touch_checked(&cache_name, &entry.version)?;
                        let bin_name = args.bin.as_deref().unwrap_or(&entry.bin_name);
                        let bin_path = cache.bin_path(&cache_name, &entry.version, bin_name);
                        return exec::exec(&bin_path, &args.tool_args);
                    }
                    return Err(e);
                }
                Err(e) => return Err(e),
            };
            let version = registry::resolve_version(&info, &args.spec.version)?;
            // Check if we already have this version cached
            if let Some(entry) = cache.lookup(&cache_name, &version.to_string())? {
                cache.touch_checked(&cache_name, &version.to_string())?;
                let bin_name = args.bin.as_deref().unwrap_or(&entry.bin_name);
                let bin_path = cache.bin_path(&cache_name, &version.to_string(), bin_name);
                return exec::exec(&bin_path, &args.tool_args);
            }
            version
        }
    };

    // Resolve binary URL
    let targets = config::TargetTriple::candidates();
    verbose!(
        "crgx: resolving binary for {} v{} ({})...",
        args.spec.name,
        resolved_version,
        targets.join(", ")
    );

    let info = registry::get_crate(&args.spec.name)?;
    let crate_version = info
        .versions
        .iter()
        .find(|v| v.num == resolved_version.to_string())
        .ok_or_else(|| error::CrgxError::VersionNotFound {
            crate_name: args.spec.name.clone(),
            version: resolved_version.to_string(),
        })?;

    // Determine bin name
    let bin_name = resolve_bin_name(&args.spec.name, args.bin.as_deref(), crate_version)?;

    let resolution = resolve::resolve(
        &args.spec.name,
        &resolved_version.to_string(),
        &targets,
        &bin_name,
        &info,
        args.allow_build,
        &args.build,
    )?;

    if resolution.target != targets[0] {
        verbose!(
            "crgx: no {} binary available, using compatible {} binary",
            targets[0],
            resolution.target
        );
    }

    // Download and extract
    verbose!("crgx: downloading from {}...", resolution.source);
    let binary_data =
        download::download_and_extract(&resolution.url, &bin_name, resolution.bin_path.as_deref())?;

    // Store in cache
    cache.store(
        &cache_name,
        &resolved_version.to_string(),
        &bin_name,
        &binary_data,
        &resolution.source,
        &resolution.target,
    )?;

    verbose!("crgx: cached {} v{}", args.spec.name, resolved_version);

    // Execute
    let bin_path = cache.bin_path(&cache_name, &resolved_version.to_string(), &bin_name);
    exec::exec(&bin_path, &args.tool_args)
}

fn resolve_bin_name(
    crate_name: &str,
    bin_override: Option<&str>,
    version: &registry::CrateVersion,
) -> Result<String, error::CrgxError> {
    if let Some(name) = bin_override {
        return Ok(name.to_string());
    }

    let bins: Vec<&str> = version.bin_names.iter().map(|s| s.as_str()).collect();

    if bins.is_empty() {
        // No bin info from registry — assume crate name
        return Ok(crate_name.to_string());
    }

    if bins.len() == 1 {
        return Ok(bins[0].to_string());
    }

    // Multiple bins — prefer one matching crate name
    if bins.contains(&crate_name) {
        return Ok(crate_name.to_string());
    }

    Err(error::CrgxError::AmbiguousBinary {
        crate_name: crate_name.to_string(),
        bins: bins.join(", "),
    })
}
