//! Private per-agent workspace management.
//!
//! Each CLI agent that needs tool-denial config gets a private directory under
//! the app's data dir: `<data_dir>/cli-workspaces/<provider id>/`.
//! Before every spawn, the required config files are (re)written there.
//!
//! **Security:** every path component (the `cli-workspaces` dir, the provider
//! subdir, every intermediate dir of each file, and every file itself) is
//! checked with `symlink_metadata` to refuse symlinks, junctions, and reparse
//! points. This prevents a local user from pre-creating a path that points
//! outside the private workspace or re-enables tools via a tampered config.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Check `path` with `symlink_metadata` (does NOT follow symlinks) and return
/// an error if it is a symlink/junction/reparse point.
/// A path that doesn't exist yet is fine.
pub fn refuse_link(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.mode() & 0o170000 == 0o120000 {
            return Err(io::Error::other(format!(
                "path is a symlink: {}",
                path.display()
            )));
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_REPARSE_POINT = 0x400 (symlink, junction, mount point)
        if meta.file_attributes() & 0x400 != 0 {
            return Err(io::Error::other(format!(
                "path is a reparse point: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

/// Write the backend's workspace files into a private directory and return the
/// working directory to use for the spawn.
///
/// `base_dir` is the app's data dir (`platform::config::data_dir()`). The
/// workspace will be created at `base_dir/cli-workspaces/<provider_id>/`.
///
/// Returns the workspace directory path on success.
pub fn prepare_workspace(
    base_dir: &Path,
    provider_id: &str,
    workspace_files: &[(&'static str, String)],
) -> io::Result<PathBuf> {
    let workspace_root = base_dir.join("cli-workspaces");
    let provider_dir = workspace_root.join(provider_id);

    // Check and create `cli-workspaces`
    if workspace_root.exists() {
        refuse_link(&workspace_root)?;
    } else {
        fs::create_dir_all(&workspace_root)?;
    }

    // Start from an EMPTY provider dir on every spawn: the CLI may also load files
    // this writer doesn't own (opencode reads `.opencode/agent/*.md`, plugins, …),
    // and a stale one could override the tool-refusing config. `remove_dir_all`
    // does not follow symlinks. `create_dir` (not `_all`) fails if anything
    // reappeared at the path between the removal and the creation.
    if provider_dir.exists() {
        refuse_link(&provider_dir)?;
        fs::remove_dir_all(&provider_dir)?;
    }
    fs::create_dir(&provider_dir)?;

    // Write each file, checking every intermediate directory
    for (rel_path, contents) in workspace_files {
        let file_path = provider_dir.join(rel_path);

        // Check each parent component
        let mut current = provider_dir.clone();
        for component in Path::new(rel_path)
            .parent()
            .into_iter()
            .flat_map(|p| p.components())
        {
            current = current.join(component);
            if current.exists() {
                refuse_link(&current)?;
            } else {
                fs::create_dir_all(&current)?;
            }
        }

        // Check and write the file
        if file_path.exists() {
            refuse_link(&file_path)?;
        }
        fs::write(&file_path, contents)?;
    }

    Ok(provider_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn workspace_files_written_with_exact_contents() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();

        let files = [(".opencode/opencode.json", r#"{"agent":"ajh"}"#.to_string())];
        let ws = prepare_workspace(base, "opencode", &files).unwrap();

        let content = fs::read_to_string(ws.join(".opencode/opencode.json")).unwrap();
        assert_eq!(content, r#"{"agent":"ajh"}"#);
    }

    #[test]
    fn pre_existing_file_with_different_contents_is_overwritten() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();

        let files = [(".opencode/opencode.json", "old".to_string())];
        prepare_workspace(base, "opencode", &files).unwrap();

        let files = [(".opencode/opencode.json", "new".to_string())];
        let ws = prepare_workspace(base, "opencode", &files).unwrap();

        let content = fs::read_to_string(ws.join(".opencode/opencode.json")).unwrap();
        assert_eq!(content, "new");
    }

    /// A file the writer doesn't own (e.g. a planted opencode agent definition)
    /// must not survive into the next run.
    #[test]
    fn files_the_writer_does_not_own_are_removed() {
        let tmp = tempfile::tempdir().unwrap();
        let files = [(".opencode/opencode.json", "{}".to_string())];
        let ws = prepare_workspace(tmp.path(), "opencode", &files).unwrap();
        let planted = ws.join(".opencode/agent/evil.md");
        fs::create_dir_all(planted.parent().unwrap()).unwrap();
        fs::write(&planted, "permission: allow").unwrap();

        prepare_workspace(tmp.path(), "opencode", &files).unwrap();
        assert!(!planted.exists(), "stale file survived");
        assert!(ws.join(".opencode/opencode.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_workspace_dir_is_refused() {
        use std::os::unix::fs::symlink;
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();

        // Create a real dir and a symlink to it outside the base
        let real = tmp.path().join("real_workspace");
        fs::create_dir_all(&real).unwrap();
        let link = base.join("cli-workspaces");
        symlink(&real, &link).unwrap();

        let files = [(".opencode/opencode.json", "{}".to_string())];
        let err = prepare_workspace(base, "opencode", &files).unwrap_err();
        assert!(err.to_string().contains("symlink"));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_leaf_file_is_refused() {
        use std::os::unix::fs::symlink;
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();

        // Create a real file and symlink to it
        let real_file = tmp.path().join("real_config.json");
        fs::write(&real_file, "{}").unwrap();
        let link = base.join("cli-workspaces/opencode/.opencode/opencode.json");
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        symlink(&real_file, &link).unwrap();

        let files = [(".opencode/opencode.json", "{}".to_string())];
        let err = prepare_workspace(base, "opencode", &files).unwrap_err();
        assert!(err.to_string().contains("symlink"));
    }

    /// A directory junction: the Windows reparse point that needs no admin
    /// rights to create, so this runs on every Windows dev machine and CI runner.
    #[cfg(windows)]
    fn junction(link: &std::path::Path, target: &std::path::Path) {
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
    fn junctioned_workspace_root_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("elsewhere");
        fs::create_dir_all(&real).unwrap();
        junction(&tmp.path().join("cli-workspaces"), &real);

        let files = [(".opencode/opencode.json", "{}".to_string())];
        let err = prepare_workspace(tmp.path(), "opencode", &files).unwrap_err();
        assert!(err.to_string().contains("reparse point"), "{err}");
        assert!(
            !real.join("opencode").exists(),
            "nothing written through the junction"
        );
    }

    #[cfg(windows)]
    #[test]
    fn junction_planted_inside_the_workspace_is_wiped_without_following_it() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("elsewhere");
        fs::create_dir_all(&real).unwrap();
        fs::write(real.join("keep.txt"), "target data").unwrap();
        let provider = tmp.path().join("cli-workspaces").join("opencode");
        fs::create_dir_all(&provider).unwrap();
        junction(&provider.join(".opencode"), &real);

        let files = [(".opencode/opencode.json", "{}".to_string())];
        let ws = prepare_workspace(tmp.path(), "opencode", &files).unwrap();

        // The junction's target is untouched: nothing deleted, nothing written.
        assert_eq!(
            fs::read_to_string(real.join("keep.txt")).unwrap(),
            "target data"
        );
        assert!(!real.join("opencode.json").exists());
        // The config landed in a real directory, not through a link.
        refuse_link(&ws.join(".opencode")).unwrap();
        assert!(ws.join(".opencode/opencode.json").exists());
    }
}
