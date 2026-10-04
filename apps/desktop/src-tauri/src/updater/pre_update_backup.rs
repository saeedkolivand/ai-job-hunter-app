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
mod tests;
