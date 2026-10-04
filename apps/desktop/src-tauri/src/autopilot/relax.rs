//! The one-shot migration that loosens autopilots saved with the old
//! auto-prefilled restrictive filters.

use std::path::PathBuf;

use super::{Autopilot, AutopilotStore};

/// Sidecar marker that records the legacy-filter loosen migration ran. Lives
/// beside `autopilots.json` in the data dir; its presence makes
/// [`AutopilotStore::relax_legacy_filters_once`] a no-op on subsequent launches.
pub(super) const RELAX_MARKER_FILE: &str = "autopilot_relax_v1.done";

/// Surgically loosen one autopilot's auto-prefilled restrictive filters in place.
/// Kills the zero-jobs bug while preserving deliberate user customizations:
///
/// - `filter.keywords` → `None` **only on a legacy record** (see below),
/// - `filter.min_match_score` → `0.0` **only if** it is still the old auto
///   default `50.0`,
/// - `target.date_filter` → `None` **only if** it is still the old auto default
///   `Some("24h")`.
///
/// Everything else (`exclude_keywords`, `query`, `location`, `country_code`,
/// `boards`, `pages`, `work_types`, `top_n`) is left untouched. Pure + filesystem-
/// free so it is unit-testable on a bare `&mut Autopilot`.
///
/// **Idempotency guarantee.** "Legacy" is decided up front from the two *sentinel*
/// fields (`min_match_score == 50.0` OR `date_filter == Some("24h")`) — the
/// prefilled `keywords` clear is gated on that flag, NOT applied unconditionally.
/// So a record that's already been relaxed (score `0.0`, date `None`) is NOT
/// legacy → the whole function is a no-op → re-running can never erase keywords
/// the user added after the first relaxation. This matters because the done-marker
/// write in [`AutopilotStore::relax_legacy_filters_once`] is best-effort
/// (`.ok()`-swallowed): if it fails, the migration re-runs on next launch, and
/// this no-op-on-relaxed property is what makes that rerun safe. The marker is now
/// purely an optimization, not a correctness gate.
///
/// Narrow accepted gap: a record with prefilled keywords where the user ALSO
/// changed *both* the score (≠50) *and* the date (≠"24h") reads as non-legacy, so
/// its keywords are kept. That's rare and diagnosable, and erring toward keeping
/// user data is the safe direction.
pub(crate) fn relax_legacy_filters(ap: &mut Autopilot) {
    // Decide legacy-ness from the sentinels BEFORE mutating them, so the decision
    // can't be invalidated by our own resets below.
    let was_legacy =
        ap.filter.min_match_score == 50.0 || ap.target.date_filter.as_deref() == Some("24h");

    if was_legacy {
        // Only legacy records carry the auto-prefilled keyword list manual search
        // never applies; clearing it on an already-relaxed record would erase
        // user-added keywords on a migration rerun.
        ap.filter.keywords = None;
    }

    if ap.filter.min_match_score == 50.0 {
        ap.filter.min_match_score = 0.0;
    }

    // `"24h"` is the ONLY restrictive legacy auto-default: the pre-#483 wizard's
    // `buildDefaults` set `dateFilter: '24h'`, while "any time" persisted as `None`
    // (`wizardStateToPayload` maps `'' → undefined`). A user-picked `'week'`/
    // `'month'` is therefore deliberate and left untouched.
    if ap.target.date_filter.as_deref() == Some("24h") {
        ap.target.date_filter = None;
    }
}

impl AutopilotStore {
    /// One-shot, idempotent migration that loosens autopilots saved with the old
    /// auto-prefilled restrictive filters (the cause of "autopilot returns ZERO
    /// jobs while manual search returns jobs for the same query"). Runs once per
    /// install, gated by a sidecar marker file beside `autopilots.json` — NOT a
    /// schema_version field, so the persisted/IPC shape is unchanged and gen:ipc
    /// can't drift.
    ///
    /// If the marker already exists this returns immediately. Otherwise it loads
    /// every autopilot, applies [`relax_legacy_filters`] to each, saves once, and
    /// writes the marker. Synchronous file IO — call it from the setup path, never
    /// from an async worker.
    ///
    /// Lock safety: [`Self::load`] takes `self.cache.lock()` but drops the guard
    /// before returning a cloned map, and [`Self::save`] re-takes the lock. This
    /// method only ever holds the owned clone from `load()` across the `save()`
    /// call — no `load()` guard is alive when `save()` re-locks, so there is no
    /// re-entrant lock / deadlock.
    pub fn relax_legacy_filters_once(&self) {
        let marker = self
            .data_file
            .parent()
            .map(|p| p.join(RELAX_MARKER_FILE))
            .unwrap_or_else(|| PathBuf::from(RELAX_MARKER_FILE));
        if marker.exists() {
            return;
        }

        let mut map = self.load(); // owned clone; the cache guard is already dropped
        for ap in map.values_mut() {
            relax_legacy_filters(ap);
        }

        // Persist FIRST and observe the result. Only write the done-marker once the
        // relaxed data is known to have hit disk — otherwise a save failure plus a
        // successful marker write would leave autopilots restrictive forever
        // ("done" but never relaxed). On a save error we skip the marker, the cache
        // stays as-loaded, and the next launch retries (harmless — the pass is
        // idempotent). `write_to_disk` returns Ok on the no-op path too (state
        // already persisted), which is also a valid "done" condition.
        let persisted = self.write_to_disk(&map);
        // Keep the in-memory cache consistent with whatever we just (attempted to)
        // persist; on success this is the relaxed map, mirroring `save`.
        *self.cache.lock() = Some(map);

        if persisted.is_ok() {
            // Mark done even if some autopilots were already loose: the goal is to
            // run the loosen pass exactly once, not to gate on whether it changed
            // anything.
            if let Some(parent) = self.data_file.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::write(&marker, b"1").ok();
        }
    }
}
