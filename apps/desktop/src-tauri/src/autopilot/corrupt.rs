//! Loading `autopilots.json` without ever destroying it. Kept out of
//! `autopilot/mod.rs` for the R8 module-size cap.
//!
//! Three outcomes, and only one of them moves the file:
//! - missing: first run, start empty;
//! - present but its CONTENT is not a JSON array (e.g. all NUL bytes after a
//!   crash mid-write, #1274): corrupt. Move it aside to a free
//!   `autopilots.json.corrupt[.N]` slot, start empty, and let saves proceed;
//! - present but unreadable for any other reason (a sharing violation while
//!   antivirus or a backup tool holds it, permissions): NOT corruption. Leave it
//!   where it is, start empty in memory, and block saves so the empty map can
//!   never overwrite a good file.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use super::{Autopilot, AutopilotStore};
use crate::observability::sanitize_reason;

/// Backup slots: `.corrupt`, then `.corrupt.1` … `.corrupt.9`. An older backup
/// is never overwritten; with every slot taken, saves are blocked instead.
const BACKUP_SLOTS: usize = 10;

pub(super) struct LoadOutcome {
    pub(super) map: HashMap<String, Autopilot>,
    pub(super) block_save: bool,
}

/// The first backup name not already taken, or `None` when all are.
fn free_backup_slot(data_file: &Path) -> Option<PathBuf> {
    (0..BACKUP_SLOTS)
        .map(|n| match n {
            0 => data_file.with_extension("json.corrupt"),
            n => data_file.with_extension(format!("json.corrupt.{n}")),
        })
        .find(|p| p.symlink_metadata().is_err())
}

/// Move a corrupt file aside. Returns whether it was moved; the caller blocks
/// saves when it wasn't, so the only copy is never overwritten. Logs the file
/// NAME only (path privacy).
fn back_up_corrupt(data_file: &Path, reason: &str) -> bool {
    let moved_to =
        free_backup_slot(data_file).filter(|backup| std::fs::rename(data_file, backup).is_ok());
    let backup_name = moved_to
        .as_deref()
        .and_then(Path::file_name)
        .map(|n| n.to_string_lossy().into_owned());
    log::error!(
        "[autopilot] autopilots.json is corrupt ({}); backed_up_as={}",
        sanitize_reason(reason),
        backup_name.as_deref().unwrap_or("NONE, saves blocked")
    );
    moved_to.is_some()
}

fn parse_records(raw: Vec<serde_json::Value>) -> HashMap<String, Autopilot> {
    // Per-record tolerant parse: one record with an unknown/future field value
    // drops only itself instead of failing the whole file.
    let mut dropped = 0usize;
    let mut map: HashMap<String, Autopilot> = raw
        .into_iter()
        .filter_map(|v| match serde_json::from_value::<Autopilot>(v) {
            Ok(ap) => Some((ap.id.clone(), ap)),
            Err(e) => {
                dropped += 1;
                log::warn!("[autopilot] dropping unparseable record: {e}");
                None
            }
        })
        .collect();
    if dropped > 0 {
        log::warn!("[autopilot] load: dropped {dropped} unparseable record(s)");
    }
    super::strip_board_health(map.values_mut());
    map
}

impl AutopilotStore {
    pub(super) fn load_with_corrupt_handling(&self) -> LoadOutcome {
        let empty = |block_save| LoadOutcome {
            map: HashMap::new(),
            block_save,
        };
        let contents = match std::fs::read_to_string(&self.data_file) {
            Ok(contents) => contents,
            Err(e) if e.kind() == ErrorKind::NotFound => return empty(false),
            // Bytes that aren't UTF-8 are damaged content, same as bad JSON.
            Err(e) if e.kind() == ErrorKind::InvalidData => {
                return empty(!back_up_corrupt(&self.data_file, &e.to_string()));
            }
            Err(e) => {
                // An io::Error can carry the absolute path: keep only the safe part.
                log::error!(
                    "[autopilot] autopilots.json could not be read ({}); left in place, \
                     saves blocked for this session",
                    sanitize_reason(&e.to_string())
                );
                return empty(true);
            }
        };
        match serde_json::from_str::<Vec<serde_json::Value>>(&contents) {
            Ok(raw) => LoadOutcome {
                map: parse_records(raw),
                block_save: false,
            },
            Err(e) => empty(!back_up_corrupt(&self.data_file, &e.to_string())),
        }
    }
}
