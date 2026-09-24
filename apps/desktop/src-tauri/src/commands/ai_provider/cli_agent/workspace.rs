//! Private per-spawn workspace for CLI agents that deny tools through config
//! files in their working directory.
//!
//! Every spawn gets its OWN fresh directory,
//! `<data_dir>/cli-workspaces/<provider id>/<run id>/`, removed when the returned
//! [`Workspace`] is dropped (after the child exits). Concurrent generations of
//! the same provider therefore never share a directory: a shared one let one
//! spawn delete the config another had not read yet, and for opencode a
//! missing config means the shell tool runs.
//!
//! **Links:** `cli-workspaces` and the provider dir are checked with
//! `symlink_metadata` and refused if they are a symlink, junction or other
//! reparse point, so the tree can't be redirected outside the data dir. The run
//! dir itself is created with `create_dir`, which fails if anything already
//! exists at that path, so nothing planted can sit inside it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A run dir older than this is left over from a crash and may be removed by
/// the next spawn. Far longer than any live run (the one-shot timeout is five
/// minutes and a streamed stage is bounded by its own deadline).
const STALE_AFTER: Duration = Duration::from_secs(24 * 60 * 60);

/// The run's private directory; removed on drop.
pub struct Workspace {
    dir: PathBuf,
}

impl Workspace {
    pub fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        // `remove_dir_all` removes a link without following it.
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Refuse `path` if it is a symlink, junction or other reparse point. The
/// error names the file only, never the full path (AGENTS.md path privacy).
pub fn refuse_link(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    #[cfg(windows)]
    let is_link = {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_REPARSE_POINT: symlinks, junctions, mount points.
        meta.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let is_link = meta.file_type().is_symlink();
    if is_link {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Err(io::Error::other(format!(
            "CLI workspace path is a link or reparse point: {name}"
        )));
    }
    Ok(())
}

/// Create `dir` if missing, then refuse it if it is (or has become) a link.
fn ensure_real_dir(dir: &Path) -> io::Result<()> {
    match fs::create_dir(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => refuse_link(dir),
        Err(e) => Err(e),
    }
}

/// Best-effort removal of run dirs a crash left behind.
fn remove_stale_runs(provider_dir: &Path) {
    let Ok(entries) = fs::read_dir(provider_dir) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let Ok(meta) = entry.path().symlink_metadata() else {
            continue;
        };
        let old = meta
            .modified()
            .ok()
            .and_then(|m| now.duration_since(m).ok())
            .is_some_and(|age| age > STALE_AFTER);
        if old {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

/// A run id unique within this machine: process id, wall-clock nanos, and a
/// per-process counter (so two spawns in the same nanosecond still differ).
fn run_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!(
        "{}-{nanos}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

/// Write `files` into a fresh run dir under
/// `base_dir/cli-workspaces/<provider_id>/` and return it.
///
/// `base_dir` is the app's data dir (`platform::config::data_dir()`); a
/// parameter so tests can use a temp dir.
pub fn prepare_workspace(
    base_dir: &Path,
    provider_id: &str,
    files: &[(&'static str, String)],
) -> io::Result<Workspace> {
    let root = base_dir.join("cli-workspaces");
    ensure_real_dir(&root)?;
    let provider_dir = root.join(provider_id);
    ensure_real_dir(&provider_dir)?;
    remove_stale_runs(&provider_dir);

    let dir = provider_dir.join(run_id());
    fs::create_dir(&dir)?;
    // From here on the guard owns the dir, so a failed write still cleans up.
    let workspace = Workspace { dir };
    for (rel_path, contents) in files {
        let file = workspace.dir.join(rel_path);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&file, contents)?;
    }
    Ok(workspace)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opencode_files(contents: &str) -> [(&'static str, String); 1] {
        [(".opencode/opencode.json", contents.to_string())]
    }

    #[test]
    fn files_are_written_with_exact_contents() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = prepare_workspace(tmp.path(), "opencode", &opencode_files("{\"a\":1}")).unwrap();
        assert_eq!(
            fs::read_to_string(ws.path().join(".opencode/opencode.json")).unwrap(),
            "{\"a\":1}"
        );
    }

    /// The race CodeRabbit found on #1275: a second spawn of the same provider
    /// must not touch the first spawn's config while the first is still running.
    #[test]
    fn concurrent_spawns_get_separate_dirs_and_keep_their_config() {
        let tmp = tempfile::tempdir().unwrap();
        let first = prepare_workspace(tmp.path(), "opencode", &opencode_files("first")).unwrap();
        let second = prepare_workspace(tmp.path(), "opencode", &opencode_files("second")).unwrap();

        assert_ne!(first.path(), second.path());
        assert_eq!(
            fs::read_to_string(first.path().join(".opencode/opencode.json")).unwrap(),
            "first"
        );
        drop(second);
        assert!(first.path().join(".opencode/opencode.json").exists());
    }

    #[test]
    fn the_run_dir_is_removed_on_drop() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = prepare_workspace(tmp.path(), "opencode", &opencode_files("{}")).unwrap();
        let dir = ws.path().to_path_buf();
        drop(ws);
        assert!(!dir.exists());
    }

    #[test]
    fn each_run_starts_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let first = prepare_workspace(tmp.path(), "opencode", &opencode_files("{}")).unwrap();
        fs::write(first.path().join("planted.md"), "permission: allow").unwrap();

        let second = prepare_workspace(tmp.path(), "opencode", &opencode_files("{}")).unwrap();
        assert!(!second.path().join("planted.md").exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_workspace_root_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("elsewhere");
        fs::create_dir_all(&real).unwrap();
        std::os::unix::fs::symlink(&real, tmp.path().join("cli-workspaces")).unwrap();

        let err = prepare_workspace(tmp.path(), "opencode", &opencode_files("{}"))
            .err()
            .unwrap();
        assert!(err.to_string().contains("link"), "{err}");
        assert!(
            fs::read_dir(&real).unwrap().next().is_none(),
            "nothing written through the link"
        );
    }

    /// A directory junction: the Windows reparse point that needs no admin
    /// rights, so this runs on every Windows machine and CI runner.
    #[cfg(windows)]
    fn junction(link: &Path, target: &Path) {
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "mklink /J failed");
    }

    #[cfg(windows)]
    #[test]
    fn a_junctioned_workspace_root_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("elsewhere");
        fs::create_dir_all(&real).unwrap();
        junction(&tmp.path().join("cli-workspaces"), &real);

        let err = prepare_workspace(tmp.path(), "opencode", &opencode_files("{}"))
            .err()
            .unwrap();
        assert!(err.to_string().contains("reparse point"), "{err}");
        // Path privacy: the error names the file, never the full path.
        let tmp_path = tmp.path().to_string_lossy().to_string();
        assert!(
            !err.to_string().contains(&tmp_path),
            "full path leaked: {err}"
        );
        assert!(
            fs::read_dir(&real).unwrap().next().is_none(),
            "nothing written through the junction"
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_junctioned_provider_dir_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("elsewhere");
        fs::create_dir_all(&real).unwrap();
        fs::create_dir_all(tmp.path().join("cli-workspaces")).unwrap();
        junction(&tmp.path().join("cli-workspaces").join("opencode"), &real);

        let err = prepare_workspace(tmp.path(), "opencode", &opencode_files("{}"))
            .err()
            .unwrap();
        assert!(err.to_string().contains("reparse point"), "{err}");
        // Path privacy: the error names the file, never the full path.
        let tmp_path = tmp.path().to_string_lossy().to_string();
        assert!(
            !err.to_string().contains(&tmp_path),
            "full path leaked: {err}"
        );
        assert!(
            fs::read_dir(&real).unwrap().next().is_none(),
            "nothing written through the junction"
        );
    }
}
