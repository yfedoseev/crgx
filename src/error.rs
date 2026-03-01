use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum CrgxError {
    #[error("crate not found on crates.io: {0}")]
    CrateNotFound(String),

    #[error("version {version} not found for crate {crate_name}")]
    VersionNotFound { crate_name: String, version: String },

    #[error(
        "no pre-built binary found for {crate_name} v{version} ({target})\n  Run with --allow-build to compile from source."
    )]
    NoBinaryFound {
        crate_name: String,
        version: String,
        target: String,
    },

    #[error("binary {bin_name} not found in downloaded archive for {crate_name}")]
    BinaryNotInArchive {
        crate_name: String,
        bin_name: String,
    },

    #[error("crate {crate_name} has multiple binaries: {bins}. Use --bin <name> to select one.")]
    AmbiguousBinary { crate_name: String, bins: String },

    #[error("crate {0} is not cached. Run without --no-install to download it.")]
    NotCached(String),

    #[error("cannot prompt for confirmation (not a TTY). Use -y to auto-confirm.")]
    NotATty,

    #[error("download declined by user")]
    DownloadDeclined,

    #[error("HTTP error fetching {url}: {status}")]
    HttpStatus { url: String, status: u16 },

    #[error("network error: {0}")]
    Network(String),

    #[error("failed to extract archive: {0}")]
    Extraction(String),

    #[error("cache directory error: {0}")]
    CacheDir(String),

    #[error("cargo build failed: {0}")]
    CargoBuild(String),

    #[error("failed to execute {path}: {source}")]
    Exec {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("{0}")]
    Other(String),
}

impl From<std::io::Error> for CrgxError {
    fn from(e: std::io::Error) -> Self {
        CrgxError::Other(e.to_string())
    }
}
