use crate::config::CrateSpec;

/// Parsed CLI command.
#[derive(Debug)]
pub enum Command {
    /// Run a crate binary.
    Run(RunArgs),
    /// Show cached binaries.
    CacheList,
    /// Remove all cached binaries.
    CacheClean,
    /// Print cache directory path.
    CacheDir,
    /// Show help text.
    Help,
    /// Show version.
    Version,
}

/// Arguments for the Run command.
#[derive(Debug)]
pub struct RunArgs {
    pub spec: CrateSpec,
    pub yes: bool,
    pub allow_build: bool,
    pub bin: Option<String>,
    pub no_install: bool,
    pub tool_args: Vec<String>,
}

/// Parse CLI arguments using manual arg splitting.
///
/// This avoids clap intercepting `--help`/`--version` meant for the wrapped tool.
/// Algorithm:
/// 1. Skip argv[0]
/// 2. Consume known crgx flags left-to-right
/// 3. First non-flag arg = crate specifier
/// 4. Everything remaining = tool_args
pub fn parse_args(args: impl Iterator<Item = String>) -> Result<Command, String> {
    let args: Vec<String> = args.collect();

    let mut yes = false;
    let mut allow_build = false;
    let mut bin: Option<String> = None;
    let mut no_install = false;
    let mut crate_spec: Option<CrateSpec> = None;
    let mut tool_args: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        // If we already have a crate spec, everything else is tool args
        if crate_spec.is_some() {
            tool_args = args[i..].to_vec();
            break;
        }

        match arg.as_str() {
            "-y" | "--yes" => yes = true,
            "--allow-build" => allow_build = true,
            "--no-install" => no_install = true,
            "--bin" => {
                i += 1;
                if i >= args.len() {
                    return Err("--bin requires a value".into());
                }
                bin = Some(args[i].clone());
            }
            "--cache-list" => return Ok(Command::CacheList),
            "--cache-clean" => return Ok(Command::CacheClean),
            "--cache-dir" => return Ok(Command::CacheDir),
            "--help" | "-h" => return Ok(Command::Help),
            "--version" | "-V" => return Ok(Command::Version),
            _ if arg.starts_with('-') => {
                return Err(format!("unknown flag: {arg}"));
            }
            _ => {
                // First non-flag arg is the crate specifier
                crate_spec = Some(CrateSpec::parse(arg).map_err(|e| format!("invalid crate specifier '{arg}': {e}"))?);
            }
        }
        i += 1;
    }

    match crate_spec {
        Some(spec) => Ok(Command::Run(RunArgs {
            spec,
            yes,
            allow_build,
            bin,
            no_install,
            tool_args,
        })),
        None => Ok(Command::Help),
    }
}

/// Generate help text.
pub fn help_text() -> String {
    format!(
        "\
crgx {version} — run any crate binary instantly

USAGE:
    crgx [FLAGS] <crate>[@<version>] [tool-args...]

FLAGS:
    -y, --yes           Auto-confirm download prompts
    --allow-build       Allow compiling from source if no pre-built binary found
    --bin <name>        Specify which binary to run (for multi-binary crates)
    --no-install        Only run if already cached; fail otherwise

CACHE MANAGEMENT:
    --cache-list        Show cached binaries
    --cache-clean       Remove all cached binaries
    --cache-dir         Print cache directory path

VERSION SPECIFIERS:
    crgx tool           Latest (with 24h staleness check)
    crgx tool@1.2.3    Exact version (cached forever)
    crgx tool@latest   Force latest from registry

EXAMPLES:
    crgx tokei .                        Count lines of code
    crgx -y fossil-mcp scan ./project   Download and run without prompting
    crgx ripgrep@14.1.0 --help          Run specific version",
        version = env!("CARGO_PKG_VERSION")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::VersionReq;

    fn parse(args: &[&str]) -> Result<Command, String> {
        parse_args(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn parse_simple_run() {
        let cmd = parse(&["tokei", "."]).unwrap();
        match cmd {
            Command::Run(args) => {
                assert_eq!(args.spec.name, "tokei");
                assert_eq!(args.tool_args, vec!["."]);
                assert!(!args.yes);
            }
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn parse_flags_before_crate() {
        let cmd = parse(&["-y", "--allow-build", "--bin", "rg", "ripgrep@14.1.0", "--help"]).unwrap();
        match cmd {
            Command::Run(args) => {
                assert!(args.yes);
                assert!(args.allow_build);
                assert_eq!(args.bin.as_deref(), Some("rg"));
                assert_eq!(args.spec.name, "ripgrep");
                assert!(matches!(args.spec.version, VersionReq::Exact(_)));
                assert_eq!(args.tool_args, vec!["--help"]);
            }
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn parse_cache_commands() {
        assert!(matches!(parse(&["--cache-list"]).unwrap(), Command::CacheList));
        assert!(matches!(parse(&["--cache-clean"]).unwrap(), Command::CacheClean));
        assert!(matches!(parse(&["--cache-dir"]).unwrap(), Command::CacheDir));
    }

    #[test]
    fn parse_help_without_args() {
        assert!(matches!(parse(&[]).unwrap(), Command::Help));
    }

    #[test]
    fn parse_help_flag() {
        assert!(matches!(parse(&["--help"]).unwrap(), Command::Help));
    }

    #[test]
    fn parse_tool_gets_help() {
        // --help after crate spec goes to tool, not crgx
        let cmd = parse(&["tokei", "--help"]).unwrap();
        match cmd {
            Command::Run(args) => {
                assert_eq!(args.tool_args, vec!["--help"]);
            }
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn parse_no_install() {
        let cmd = parse(&["--no-install", "tokei"]).unwrap();
        match cmd {
            Command::Run(args) => {
                assert!(args.no_install);
            }
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn parse_unknown_flag() {
        assert!(parse(&["--unknown", "tokei"]).is_err());
    }

    #[test]
    fn parse_bin_missing_value() {
        assert!(parse(&["--bin"]).is_err());
    }

    #[test]
    fn parse_version_flag() {
        assert!(matches!(parse(&["--version"]).unwrap(), Command::Version));
        assert!(matches!(parse(&["-V"]).unwrap(), Command::Version));
    }

    #[test]
    fn tool_args_pass_through_flags() {
        let cmd = parse(&["tokei", "-y", "--allow-build", "--no-install"]).unwrap();
        match cmd {
            Command::Run(args) => {
                // These all go to the tool since they appear after the crate spec
                assert_eq!(args.tool_args, vec!["-y", "--allow-build", "--no-install"]);
                assert!(!args.yes); // Not consumed by crgx
            }
            _ => panic!("expected Run"),
        }
    }
}
