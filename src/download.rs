use std::io::{self, Read};
use std::path::Path;

use crate::error::CrgxError;

const USER_AGENT: &str = concat!("crgx/", env!("CARGO_PKG_VERSION"));

/// Download a URL and extract the target binary.
///
/// Returns the binary contents as bytes.
pub fn download_and_extract(
    url: &str,
    bin_name: &str,
    bin_path_hint: Option<&str>,
) -> Result<Vec<u8>, CrgxError> {
    // Handle file:// URLs (from cargo build)
    if let Some(path) = url.strip_prefix("file://") {
        return read_local_binary(path, bin_name);
    }

    let mut response = ureq::get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| match &e {
            ureq::Error::StatusCode(code) => CrgxError::HttpStatus {
                url: url.to_string(),
                status: *code,
            },
            _ => CrgxError::Network(e.to_string()),
        })?;

    let body = response
        .body_mut()
        .read_to_vec()
        .map_err(|e| CrgxError::Network(format!("download error: {e}")))?;

    let url_lower = url.to_lowercase();
    if url_lower.ends_with(".tar.gz") || url_lower.ends_with(".tgz") {
        extract_tar_gz(&body, bin_name, bin_path_hint)
    } else if url_lower.ends_with(".tar.xz") {
        extract_tar_xz(&body, bin_name, bin_path_hint)
    } else if url_lower.ends_with(".zip") {
        extract_zip(&body, bin_name, bin_path_hint)
    } else {
        // Assume raw binary
        Ok(body)
    }
}

fn read_local_binary(dir_path: &str, bin_name: &str) -> Result<Vec<u8>, CrgxError> {
    let ext = if cfg!(windows) { ".exe" } else { "" };
    let bin_path = Path::new(dir_path).join(format!("{bin_name}{ext}"));
    if bin_path.exists() {
        return std::fs::read(&bin_path).map_err(|e| CrgxError::Other(e.to_string()));
    }

    // Try without extension
    let bin_path = Path::new(dir_path).join(bin_name);
    if bin_path.exists() {
        return std::fs::read(&bin_path).map_err(|e| CrgxError::Other(e.to_string()));
    }

    Err(CrgxError::BinaryNotInArchive {
        crate_name: bin_name.to_string(),
        bin_name: bin_name.to_string(),
    })
}

fn extract_tar_gz(
    data: &[u8],
    bin_name: &str,
    bin_path_hint: Option<&str>,
) -> Result<Vec<u8>, CrgxError> {
    let gz = flate2::read::GzDecoder::new(io::Cursor::new(data));
    extract_tar(gz, bin_name, bin_path_hint)
}

fn extract_tar_xz(
    data: &[u8],
    bin_name: &str,
    bin_path_hint: Option<&str>,
) -> Result<Vec<u8>, CrgxError> {
    let xz = xz2::read::XzDecoder::new(io::Cursor::new(data));
    extract_tar(xz, bin_name, bin_path_hint)
}

fn extract_tar<R: Read>(
    reader: R,
    bin_name: &str,
    bin_path_hint: Option<&str>,
) -> Result<Vec<u8>, CrgxError> {
    let mut archive = tar::Archive::new(reader);
    let ext = if cfg!(windows) { ".exe" } else { "" };
    let bin_filename = format!("{bin_name}{ext}");

    let mut entries = archive
        .entries()
        .map_err(|e| CrgxError::Extraction(e.to_string()))?;

    // If we have a bin-dir hint, try that path first
    // We'll collect all entries and search
    let mut found: Option<Vec<u8>> = None;
    let mut fallback: Option<Vec<u8>> = None;

    while let Some(entry) = entries.next() {
        let mut entry = entry.map_err(|e| CrgxError::Extraction(e.to_string()))?;
        let path = entry
            .path()
            .map_err(|e| CrgxError::Extraction(e.to_string()))?
            .to_path_buf();

        let file_name = path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");

        // Check bin_path_hint
        if let Some(hint) = bin_path_hint {
            let hint_path = Path::new(hint);
            if path.ends_with(hint_path) {
                let mut data = Vec::new();
                entry
                    .read_to_end(&mut data)
                    .map_err(|e| CrgxError::Extraction(e.to_string()))?;
                return Ok(data);
            }
        }

        // Exact filename match (handles archives with directory prefix)
        if file_name == bin_filename {
            let mut data = Vec::new();
            entry
                .read_to_end(&mut data)
                .map_err(|e| CrgxError::Extraction(e.to_string()))?;
            // Prefer files not in deeply nested dirs
            if path.components().count() <= 2 {
                found = Some(data);
            } else if found.is_none() {
                fallback = Some(data);
            }
        } else if file_name == bin_name && ext.is_empty() {
            // Also try without extension on non-Windows
            let mut data = Vec::new();
            entry
                .read_to_end(&mut data)
                .map_err(|e| CrgxError::Extraction(e.to_string()))?;
            if fallback.is_none() {
                fallback = Some(data);
            }
        }
    }

    found.or(fallback).ok_or_else(|| CrgxError::BinaryNotInArchive {
        crate_name: bin_name.to_string(),
        bin_name: bin_filename,
    })
}

fn extract_zip(
    data: &[u8],
    bin_name: &str,
    bin_path_hint: Option<&str>,
) -> Result<Vec<u8>, CrgxError> {
    let cursor = io::Cursor::new(data);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| CrgxError::Extraction(e.to_string()))?;

    let ext = if cfg!(windows) { ".exe" } else { "" };
    let bin_filename = format!("{bin_name}{ext}");

    // If we have a bin_path_hint, try it first
    if let Some(hint) = bin_path_hint {
        if let Ok(mut file) = archive.by_name(hint) {
            let mut data = Vec::new();
            file.read_to_end(&mut data)
                .map_err(|e| CrgxError::Extraction(e.to_string()))?;
            return Ok(data);
        }
    }

    // Search for the binary by filename
    let mut found: Option<Vec<u8>> = None;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| CrgxError::Extraction(e.to_string()))?;

        let path = Path::new(file.name()).to_path_buf();
        let file_name = path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");

        if file_name == bin_filename || (ext.is_empty() && file_name == bin_name) {
            let mut data = Vec::new();
            file.read_to_end(&mut data)
                .map_err(|e| CrgxError::Extraction(e.to_string()))?;
            found = Some(data);
            break;
        }
    }

    found.ok_or_else(|| CrgxError::BinaryNotInArchive {
        crate_name: bin_name.to_string(),
        bin_name: bin_filename,
    })
}
