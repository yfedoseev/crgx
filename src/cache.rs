use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::error::CrgxError;

const STALENESS_WINDOW: Duration = Duration::from_secs(24 * 60 * 60); // 24 hours

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheMetadata {
    pub crate_name: String,
    pub version: String,
    pub bin_name: String,
    pub source: String,
    pub installed_at: u64,
    pub last_checked: u64,
    pub target: String,
}

/// A listing entry for display.
pub struct CacheEntry {
    pub crate_name: String,
    pub version: String,
    pub source: String,
}

pub struct Cache {
    root: PathBuf,
}

impl Cache {
    /// Open the cache directory, creating it if necessary.
    pub fn open() -> Result<Self, CrgxError> {
        let root = cache_dir()?;
        fs::create_dir_all(root.join("bin"))
            .map_err(|e| CrgxError::CacheDir(e.to_string()))?;
        Ok(Cache { root })
    }

    /// Returns the root cache directory path.
    pub fn dir(&self) -> &Path {
        &self.root
    }

    /// Path to a cached binary.
    pub fn bin_path(&self, crate_name: &str, version: &str, bin_name: &str) -> PathBuf {
        let ext = if cfg!(windows) { ".exe" } else { "" };
        self.root
            .join("bin")
            .join(crate_name)
            .join(version)
            .join(format!("{bin_name}{ext}"))
    }

    fn metadata_path(&self, crate_name: &str, version: &str) -> PathBuf {
        self.root
            .join("bin")
            .join(crate_name)
            .join(version)
            .join("metadata.json")
    }

    /// Look up a specific version in the cache.
    pub fn lookup(&self, crate_name: &str, version: &str) -> Result<Option<CacheMetadata>, CrgxError> {
        let path = self.metadata_path(crate_name, version);
        if !path.exists() {
            return Ok(None);
        }
        let data = fs::read_to_string(&path)?;
        let meta: CacheMetadata =
            serde_json::from_str(&data).map_err(|e| CrgxError::CacheDir(e.to_string()))?;
        Ok(Some(meta))
    }

    /// Find the latest cached version of a crate (by semver).
    pub fn find_latest_cached(&self, crate_name: &str) -> Result<Option<CacheMetadata>, CrgxError> {
        let crate_dir = self.root.join("bin").join(crate_name);
        if !crate_dir.exists() {
            return Ok(None);
        }

        let mut best: Option<(semver::Version, CacheMetadata)> = None;
        for entry in fs::read_dir(&crate_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let ver_str = entry.file_name().to_string_lossy().to_string();
            let Ok(ver) = semver::Version::parse(&ver_str) else {
                continue;
            };
            let meta_path = entry.path().join("metadata.json");
            if !meta_path.exists() {
                continue;
            }
            let data = fs::read_to_string(&meta_path)?;
            let Ok(meta) = serde_json::from_str::<CacheMetadata>(&data) else {
                continue;
            };
            if best.as_ref().is_none_or(|(best_ver, _)| ver > *best_ver) {
                best = Some((ver, meta));
            }
        }
        Ok(best.map(|(_, meta)| meta))
    }

    /// Check if a cached version is stale (last_checked > 24h ago).
    pub fn is_stale(&self, crate_name: &str, version: &str) -> Result<bool, CrgxError> {
        let meta = match self.lookup(crate_name, version)? {
            Some(m) => m,
            None => return Ok(true),
        };
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Ok(now.saturating_sub(meta.last_checked) > STALENESS_WINDOW.as_secs())
    }

    /// Update the last_checked timestamp for a cached entry.
    pub fn touch_checked(&self, crate_name: &str, version: &str) -> Result<(), CrgxError> {
        let path = self.metadata_path(crate_name, version);
        if !path.exists() {
            return Ok(());
        }
        let data = fs::read_to_string(&path)?;
        let mut meta: CacheMetadata =
            serde_json::from_str(&data).map_err(|e| CrgxError::CacheDir(e.to_string()))?;
        meta.last_checked = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let json = serde_json::to_string_pretty(&meta)
            .map_err(|e| CrgxError::CacheDir(e.to_string()))?;
        fs::write(&path, json)?;
        Ok(())
    }

    /// Store a binary in the cache (atomic write).
    pub fn store(
        &self,
        crate_name: &str,
        version: &str,
        bin_name: &str,
        binary_data: &[u8],
        source: &str,
    ) -> Result<(), CrgxError> {
        let dir = self.root.join("bin").join(crate_name).join(version);
        fs::create_dir_all(&dir)?;

        let bin_path = self.bin_path(crate_name, version, bin_name);

        // Atomic write: write to temp file, then rename
        let tmp_path = dir.join(format!(".{bin_name}.tmp"));
        {
            let mut f = fs::File::create(&tmp_path)?;
            f.write_all(binary_data)?;
            f.flush()?;
        }
        fs::rename(&tmp_path, &bin_path)?;

        // Set executable permission on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&bin_path, fs::Permissions::from_mode(0o755))?;
        }

        // Write metadata
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let meta = CacheMetadata {
            crate_name: crate_name.to_string(),
            version: version.to_string(),
            bin_name: bin_name.to_string(),
            source: source.to_string(),
            installed_at: now,
            last_checked: now,
            target: crate::config::TargetTriple::host().to_string(),
        };
        let json = serde_json::to_string_pretty(&meta)
            .map_err(|e| CrgxError::CacheDir(e.to_string()))?;
        let meta_path = self.metadata_path(crate_name, version);
        fs::write(&meta_path, json)?;

        Ok(())
    }

    /// List all cached entries.
    pub fn list(&self) -> Result<Vec<CacheEntry>, CrgxError> {
        let bin_dir = self.root.join("bin");
        if !bin_dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = Vec::new();
        for crate_entry in fs::read_dir(&bin_dir)? {
            let crate_entry = crate_entry?;
            if !crate_entry.file_type()?.is_dir() {
                continue;
            }
            let crate_name = crate_entry.file_name().to_string_lossy().to_string();
            for ver_entry in fs::read_dir(crate_entry.path())? {
                let ver_entry = ver_entry?;
                if !ver_entry.file_type()?.is_dir() {
                    continue;
                }
                let version = ver_entry.file_name().to_string_lossy().to_string();
                let meta_path = ver_entry.path().join("metadata.json");
                let source = if meta_path.exists() {
                    let data = fs::read_to_string(&meta_path)?;
                    serde_json::from_str::<CacheMetadata>(&data)
                        .map(|m| m.source)
                        .unwrap_or_else(|_| "unknown".into())
                } else {
                    "unknown".into()
                };
                entries.push(CacheEntry {
                    crate_name: crate_name.clone(),
                    version,
                    source,
                });
            }
        }
        entries.sort_by(|a, b| a.crate_name.cmp(&b.crate_name).then(a.version.cmp(&b.version)));
        Ok(entries)
    }

    /// Remove all cached binaries. Returns count of removed entries.
    pub fn clean(&self) -> Result<usize, CrgxError> {
        let bin_dir = self.root.join("bin");
        if !bin_dir.exists() {
            return Ok(0);
        }

        let mut count = 0;
        for crate_entry in fs::read_dir(&bin_dir)? {
            let crate_entry = crate_entry?;
            if crate_entry.file_type()?.is_dir() {
                for ver_entry in fs::read_dir(crate_entry.path())? {
                    let ver_entry = ver_entry?;
                    if ver_entry.file_type()?.is_dir() {
                        count += 1;
                    }
                }
                fs::remove_dir_all(crate_entry.path())?;
            }
        }
        Ok(count)
    }
}

fn cache_dir() -> Result<PathBuf, CrgxError> {
    let dir = dirs::cache_dir()
        .ok_or_else(|| CrgxError::CacheDir("could not determine cache directory".into()))?
        .join("crgx");
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_store_and_lookup() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = Cache {
            root: tmp.path().to_path_buf(),
        };
        fs::create_dir_all(cache.root.join("bin")).unwrap();

        cache
            .store("mycrate", "1.0.0", "mybin", b"fake-binary", "test")
            .unwrap();

        let meta = cache.lookup("mycrate", "1.0.0").unwrap().unwrap();
        assert_eq!(meta.crate_name, "mycrate");
        assert_eq!(meta.version, "1.0.0");
        assert_eq!(meta.bin_name, "mybin");

        let bin_path = cache.bin_path("mycrate", "1.0.0", "mybin");
        assert!(bin_path.exists());
        assert_eq!(fs::read(&bin_path).unwrap(), b"fake-binary");
    }

    #[test]
    fn cache_find_latest() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = Cache {
            root: tmp.path().to_path_buf(),
        };
        fs::create_dir_all(cache.root.join("bin")).unwrap();

        cache
            .store("mycrate", "1.0.0", "mybin", b"v1", "test")
            .unwrap();
        cache
            .store("mycrate", "2.0.0", "mybin", b"v2", "test")
            .unwrap();
        cache
            .store("mycrate", "1.5.0", "mybin", b"v1.5", "test")
            .unwrap();

        let latest = cache.find_latest_cached("mycrate").unwrap().unwrap();
        assert_eq!(latest.version, "2.0.0");
    }

    #[test]
    fn cache_staleness() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = Cache {
            root: tmp.path().to_path_buf(),
        };
        fs::create_dir_all(cache.root.join("bin")).unwrap();

        cache
            .store("mycrate", "1.0.0", "mybin", b"bin", "test")
            .unwrap();

        // Just stored — should not be stale
        assert!(!cache.is_stale("mycrate", "1.0.0").unwrap());

        // Manually set last_checked to 25h ago
        let path = cache.metadata_path("mycrate", "1.0.0");
        let data = fs::read_to_string(&path).unwrap();
        let mut meta: CacheMetadata = serde_json::from_str(&data).unwrap();
        meta.last_checked -= 25 * 3600;
        fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();

        assert!(cache.is_stale("mycrate", "1.0.0").unwrap());
    }

    #[test]
    fn cache_list_and_clean() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = Cache {
            root: tmp.path().to_path_buf(),
        };
        fs::create_dir_all(cache.root.join("bin")).unwrap();

        cache
            .store("crate_a", "1.0.0", "a", b"a", "test")
            .unwrap();
        cache
            .store("crate_b", "2.0.0", "b", b"b", "test")
            .unwrap();

        let entries = cache.list().unwrap();
        assert_eq!(entries.len(), 2);

        let count = cache.clean().unwrap();
        assert_eq!(count, 2);

        let entries = cache.list().unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn cache_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = Cache {
            root: tmp.path().to_path_buf(),
        };
        fs::create_dir_all(cache.root.join("bin")).unwrap();

        assert!(cache.lookup("nonexistent", "1.0.0").unwrap().is_none());
        assert!(cache.find_latest_cached("nonexistent").unwrap().is_none());
    }
}
