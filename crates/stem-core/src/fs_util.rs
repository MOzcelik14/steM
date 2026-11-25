//! Atomic filesystem write utilities.
//!
//! Writes data to a sibling temporary file, flushes via `File::sync_all()`,
//! atomically renames over the destination (without ever deleting the destination
//! first), and best-effort syncs the parent directory on Unix.

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use uuid::Uuid;

/// Atomically renames `tmp_path` onto `dest`.
///
/// On failure, `tmp_path` is removed (best-effort) and `dest` is never deleted
/// or modified, preventing data loss if `rename` fails.
pub fn atomic_rename(tmp_path: &Path, dest: &Path) -> std::io::Result<()> {
    if let Err(err) = fs::rename(tmp_path, dest) {
        let _ = fs::remove_file(tmp_path);
        return Err(err);
    }

    #[cfg(unix)]
    {
        let parent = dest
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        if let Ok(dir) = File::open(parent) {
            let _ = dir.sync_all();
        }
    }

    Ok(())
}

/// Atomically writes `data` to `dest` via a sibling temporary file, `sync_all`, and `rename`.
pub fn atomic_write_bytes(dest: &Path, data: &[u8]) -> std::io::Result<()> {
    let parent = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    let name = dest.file_name().and_then(|s| s.to_str()).unwrap_or("file");
    let tmp_path = parent.join(format!(".{}_{}.tmp", name, Uuid::new_v4()));

    let write_res = (|| -> std::io::Result<()> {
        let mut file = File::create(&tmp_path)?;
        file.write_all(data)?;
        file.sync_all()?;
        Ok(())
    })();

    if let Err(err) = write_res {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }

    atomic_rename(&tmp_path, dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_test_dir(prefix: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("{}_{}", prefix, Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_atomic_write_new_target() {
        let dir = temp_test_dir("stem_atomic_new");
        let dest = dir.join("project.stem");
        atomic_write_bytes(&dest, b"hello world").unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"hello world");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_atomic_write_overwrites_existing_target() {
        let dir = temp_test_dir("stem_atomic_overwrite");
        let dest = dir.join("project.stem");
        fs::write(&dest, b"old content").unwrap();
        atomic_write_bytes(&dest, b"new content").unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"new content");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_atomic_write_rename_failure_preserves_target_and_cleans_tmp() {
        let dir = temp_test_dir("stem_atomic_fail");
        let dest = dir.join("non_empty_dir");
        fs::create_dir_all(&dest).unwrap();
        let inner = dest.join("precious.txt");
        fs::write(&inner, b"keep me").unwrap();

        let err = atomic_write_bytes(&dest, b"replacement").unwrap_err();
        assert!(err.raw_os_error().is_some());
        assert!(dest.is_dir(), "destination directory must remain intact");
        assert_eq!(fs::read(&inner).unwrap(), b"keep me");

        // Ensure no leftover .tmp files in parent directory
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "temporary file must be cleaned up");

        let _ = fs::remove_dir_all(&dir);
    }
}
