use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Replace `path`'s contents atomically: a crash or kill at any point leaves
/// either the old file or the new one, never a truncated or zero-filled one.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    // Write to a sibling temp file in the SAME directory (rename is only atomic
    // on the same volume). Mirror the naming `postings/mod.rs` already uses
    // (`with_extension("json.tmp")`-style: for `X.json` produces `X.json.tmp`).
    // For a generic helper we append `.tmp` to the full filename.
    let mut tmp_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?
        .to_os_string();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);

    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        // fsync so a power loss / OS crash can't leave a zero-length or
        // zero-filled file; a process kill alone is already covered by the rename.
        file.sync_all()?;
        drop(file);
        // Replaces an existing file on Windows and Unix.
        fs::rename(&tmp, path)
    })();
    // On ANY failure (create, write, flush or rename): drop the temp file. The
    // original is untouched because nothing but the rename ever writes to it.
    // ponytail: the parent directory isn't fsynced, so on ext4 a power cut right
    // after the rename can still roll it back to the OLD file (never a torn one);
    // fsync the dir handle after the rename if that ever matters.
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn write_atomic_creates_new_file_with_exact_bytes() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.json");
        let data = b"hello world";

        write_atomic(&file, data).unwrap();

        assert!(file.exists());
        assert_eq!(fs::read(&file).unwrap(), data);
        // No .tmp file left behind
        assert!(!dir.path().join("test.json.tmp").exists());
    }

    #[test]
    fn write_atomic_replaces_existing_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.json");

        // Write initial content
        fs::write(&file, b"old").unwrap();

        // Replace with new content
        write_atomic(&file, b"new").unwrap();

        assert_eq!(fs::read(&file).unwrap(), b"new");
        assert!(!dir.path().join("test.json.tmp").exists());
    }

    #[test]
    fn write_atomic_leaves_no_temp_file_on_success() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.json");

        write_atomic(&file, b"data").unwrap();

        let tmp = dir.path().join("test.json.tmp");
        assert!(
            !tmp.exists(),
            "temp file must be renamed away, not left behind"
        );
    }

    /// The rename fails (the target is a non-empty directory): the error comes
    /// back, the target is untouched and no temp file is left behind.
    #[test]
    fn a_failed_rename_leaves_the_target_intact_and_no_temp() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("state.json");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("keep"), b"x").unwrap();

        assert!(write_atomic(&target, b"new").is_err());
        assert_eq!(fs::read(target.join("keep")).unwrap(), b"x");
        assert!(!dir.path().join("state.json.tmp").exists());
    }

    /// The temp file can't even be created: the original file keeps its bytes.
    #[test]
    fn a_failed_temp_write_leaves_the_original_intact() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("state.json");
        fs::write(&target, b"old").unwrap();
        fs::create_dir(dir.path().join("state.json.tmp")).unwrap();

        assert!(write_atomic(&target, b"new").is_err());
        assert_eq!(fs::read(&target).unwrap(), b"old");
    }
}
