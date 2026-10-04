use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Replace `path`'s contents atomically: a crash or kill at any point leaves
/// either the old file or the new one, never a truncated or zero-filled one.
///
/// Concurrent calls on the same `path` are safe too: each uses its own temp
/// file, so one writer can never truncate the temp another is about to rename
/// into place. The last rename wins with a complete file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    #[cfg(test)]
    if FAIL_NEXT_WRITE.with(|f| f.replace(false)) {
        return Err(io::Error::other("injected write failure (test)"));
    }
    let tmp = temp_path(path)?;
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
    // On ANY failure (create, write, flush or rename): drop OUR temp file. The
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
thread_local! {
    static FAIL_NEXT_WRITE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Test-only: make the next `write_atomic` on THIS thread fail before touching
/// the disk, so a caller's failure path can be tested while the real file stays
/// readable. (Blocking a fixed temp name no longer works: temp names are unique
/// per call.) Thread-local, so parallel tests can't trip each other.
#[cfg(test)]
pub fn fail_next_write_on_this_thread() {
    FAIL_NEXT_WRITE.with(|f| f.set(true));
}

/// A sibling temp path unique to this call: `<name>.<pid>.<n>.tmp`, in the same
/// directory because a rename is only atomic within one volume. A crash can
/// leave one behind; it is a stray file, never the target.
fn temp_path(path: &Path) -> io::Result<PathBuf> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?
        .to_os_string();
    name.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    Ok(path.with_file_name(name))
}

#[cfg(test)]
mod tests;
