use std::path::Path;

use crate::error::CrgxError;

/// Execute a binary, replacing the current process on Unix.
pub fn exec(bin_path: &Path, args: &[String]) -> Result<i32, CrgxError> {
    if !bin_path.exists() {
        return Err(CrgxError::Other(format!(
            "binary not found at {}",
            bin_path.display()
        )));
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = std::process::Command::new(bin_path).args(args).exec();
        // exec() only returns on error
        Err(CrgxError::Exec {
            path: bin_path.to_path_buf(),
            source: err,
        })
    }

    #[cfg(windows)]
    {
        let status = std::process::Command::new(bin_path)
            .args(args)
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .map_err(|e| CrgxError::Exec {
                path: bin_path.to_path_buf(),
                source: e,
            })?;
        Ok(status.code().unwrap_or(1))
    }

    #[cfg(not(any(unix, windows)))]
    {
        Err(CrgxError::Other("unsupported platform".into()))
    }
}
