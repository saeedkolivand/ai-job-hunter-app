//! A copy of the user's data taken just before an update installs (#1278).
//!
//! Atomic writes (#1276) make a kill mid-save safe; this covers what they
//! can't: the NEW version damaging data (a bad migration, a bug writing wrong
//! values). The backup is the same bundle the manual Export writes, so the
//! existing Restore reads it back.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::platform::fs::write_atomic;

/// How many pre-update backups to keep (owner's call on #1278).
const KEEP: usize = 3;
const PREFIX: &str = "pre-update-";

/// Write `bundle` to `<data_dir>/backups/pre-update/pre-update-<version>-<date>.json`
/// and prune older pre-update backups down to [`KEEP`]. The `pre-update/` folder
/// belongs to the updater alone, so pruning can never reach a file the user put
/// in `backups/`, whatever it is named.
///
/// Compact JSON on purpose: one store alone has been seen at 41 MB, and three
/// pretty-printed copies of that would cost real disk space.
pub(crate) fn write_pre_update_backup(
    data_dir: &Path,
    version: &str,
    date: &str,
    bundle: &Value,
) -> io::Result<PathBuf> {
    let dir = data_dir.join("backups").join("pre-update");
    fs::create_dir_all(&dir)?;
    // The version comes from our own build metadata, but it lands in a file name:
    // keep it to characters that are safe there.
    let version: String = version
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
        .collect();
    let path = dir.join(format!("{PREFIX}{version}-{date}.json"));
    let bytes = serde_json::to_vec(bundle).map_err(io::Error::other)?;
    write_atomic(&path, &bytes)?;
    prune(&dir);
    Ok(path)
}

/// Delete all but the newest [`KEEP`] `pre-update-*.json` files, by modified time.
/// Best-effort: a file that can't be removed is left for the next update.
fn prune(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut backups: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.starts_with(PREFIX) && name.ends_with(".json")
        })
        .filter_map(|e| {
            let meta = e.path().symlink_metadata().ok()?;
            meta.is_file()
                .then(|| meta.modified().ok().map(|m| (m, e.path())))
                .flatten()
        })
        .collect();
    backups.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    for (_, old) in backups.into_iter().skip(KEEP) {
        let _ = fs::remove_file(old);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn backup_names(data_dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(data_dir.join("backups").join("pre-update"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn writes_the_bundle_as_compact_json() {
        let tmp = tempfile::tempdir().unwrap();
        let bundle = json!({ "version": 1, "stores": { "a": [1, 2] } });
        let path = write_pre_update_backup(tmp.path(), "0.156.0", "2026-09-24", &bundle).unwrap();

        assert_eq!(
            path.file_name().unwrap().to_string_lossy(),
            "pre-update-0.156.0-2026-09-24.json"
        );
        let written = fs::read_to_string(&path).unwrap();
        assert!(!written.contains('\n'), "compact, not pretty-printed");
        assert_eq!(serde_json::from_str::<Value>(&written).unwrap(), bundle);
    }

    #[test]
    fn keeps_only_the_newest_three_and_never_touches_the_users_files() {
        let tmp = tempfile::tempdir().unwrap();
        let user_dir = tmp.path().join("backups");
        fs::create_dir_all(&user_dir).unwrap();
        // Named like a backup on purpose: it still isn't the updater's to prune.
        fs::write(user_dir.join("pre-update-notes.json"), b"{}").unwrap();

        for (i, version) in ["0.150.0", "0.151.0", "0.152.0", "0.153.0"]
            .iter()
            .enumerate()
        {
            write_pre_update_backup(tmp.path(), version, &format!("2026-09-0{i}"), &json!({}))
                .unwrap();
            // Distinct modified times, oldest first.
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        assert_eq!(
            backup_names(tmp.path()),
            vec![
                "pre-update-0.151.0-2026-09-01.json",
                "pre-update-0.152.0-2026-09-02.json",
                "pre-update-0.153.0-2026-09-03.json",
            ]
        );
        assert!(user_dir.join("pre-update-notes.json").exists());
    }

    #[test]
    fn a_version_cannot_steer_the_file_name() {
        let tmp = tempfile::tempdir().unwrap();
        let path =
            write_pre_update_backup(tmp.path(), "../../evil", "2026-09-24", &json!({})).unwrap();
        assert_eq!(
            path.parent().unwrap(),
            tmp.path().join("backups").join("pre-update")
        );
        assert_eq!(
            path.file_name().unwrap().to_string_lossy(),
            "pre-update-....evil-2026-09-24.json"
        );
    }

    #[test]
    fn fails_when_the_backups_path_is_not_a_directory() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("backups"), b"a file, not a dir").unwrap();
        assert!(write_pre_update_backup(tmp.path(), "0.1.0", "2026-09-24", &json!({})).is_err());
    }
}
