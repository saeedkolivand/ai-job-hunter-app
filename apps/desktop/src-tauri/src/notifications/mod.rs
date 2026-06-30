//! NotificationStore — the single source of truth for app notifications.
//!
//! A JSON-file-backed, capped, newest-first list of [`AppNotification`] records,
//! persisted to `<dataDir>/notifications.json`. It mirrors the persistence
//! pattern of [`crate::autopilot::AutopilotStore`] /
//! [`crate::postings::InteractionStore`]: a `data_file` path plus an in-memory
//! cache guarded by a `parking_lot::Mutex`. The mutators stay infallible: read
//! IO is swallowed via `.ok()`, and a failed save (serialize or write) is logged
//! via `log::warn!` rather than propagated.
//!
//! ## Layering
//! Pure **data + disk** — deliberately `AppHandle`-free (no Tauri imports beyond
//! what persistence needs, no OS-banner, no event emission). That push
//! orchestration (OS banner / tray / renderer event) is Phase 4 and lives in the
//! shell layer. Keeping the store pure means it is unit-testable without a Tauri
//! runtime, exactly like the other stores.
//!
//! Field names serialise in camelCase (`createdAt`, …) to match the TypeScript
//! schema the Phase-2 IPC commands will hand these records across.

use std::path::Path;
use std::path::PathBuf;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::db::now_ms;

/// Maximum notifications retained. Newest-first; pushing past the cap drops the
/// oldest. Enforced on `push` **and** defensively on load (in case the on-disk
/// file was hand-edited over the cap).
const MAX_NOTIFICATIONS: usize = 50;

/// Character bounds for persisted/displayed notification text. Clamp
/// attacker-influenceable scraped text at the store boundary — see Phase 4a
/// security review. Counted by CHARACTER (not byte) so a multi-byte UTF-8
/// codepoint is never split.
const MAX_TITLE_CHARS: usize = 200;
const MAX_BODY_CHARS: usize = 500;

// ── Data model ──────────────────────────────────────────────────────────────

/// A route the renderer navigates to when the notification is actioned. `to` is
/// an app route path; `search` is an optional query-param map. Open-typed (no
/// enum) for zero-change extensibility.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NotificationRoute {
    pub to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<Map<String, Value>>,
}

/// A persisted notification record. `kind` is an OPEN string (e.g.
/// `"autopilot.new_jobs"`, `"import.result"`) so new notification kinds need no
/// codebase change.
///
/// Note: there is intentionally **no** `osBanner` field — that is a Phase-4
/// push-orchestration parameter, not part of the persisted record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppNotification {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    /// Epoch millis (matches the repo's `now_ms` convention).
    pub created_at: u64,
    pub read: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<NotificationRoute>,
}

/// The `push` input: a notification before the store assigns its `id`,
/// `created_at`, and `read = false`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewNotification {
    pub kind: String,
    pub title: String,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<NotificationRoute>,
}

// ── Store ───────────────────────────────────────────────────────────────────

/// JSON-file-backed, capped, newest-first notification store.
pub struct NotificationStore {
    data_file: PathBuf,
    cache: Mutex<Option<Vec<AppNotification>>>,
}

impl NotificationStore {
    pub fn new(data_dir: &Path) -> Self {
        std::fs::create_dir_all(data_dir).ok();
        Self {
            data_file: data_dir.join("notifications.json"),
            cache: Mutex::new(None),
        }
    }

    /// Create a notification: assign `id` + `created_at`, set `read = false`,
    /// prepend (newest-first), trim to the cap (dropping the oldest), persist,
    /// and return the created record.
    pub fn push(&self, input: NewNotification) -> AppNotification {
        // Clamp at the source-of-truth boundary so every current and future
        // source is bounded in one place. Char-wise to keep UTF-8 intact.
        let title = input
            .title
            .chars()
            .take(MAX_TITLE_CHARS)
            .collect::<String>();
        let body = input.body.chars().take(MAX_BODY_CHARS).collect::<String>();
        let notification = AppNotification {
            id: Uuid::new_v4().to_string(),
            kind: input.kind,
            title,
            body,
            created_at: now_ms(),
            read: false,
            route: input.route,
        };
        let mut all = self.load();
        all.insert(0, notification.clone());
        all.truncate(MAX_NOTIFICATIONS);
        self.save(all);
        notification
    }

    /// All notifications, newest-first.
    pub fn list(&self) -> Vec<AppNotification> {
        self.load()
    }

    /// Mark a single notification read. Returns `true` if one was found and its
    /// `read` flag changed (already-read → `false`); persists on change.
    pub fn mark_read(&self, id: &str) -> bool {
        let mut all = self.load();
        let mut changed = false;
        if let Some(n) = all.iter_mut().find(|n| n.id == id) {
            if !n.read {
                n.read = true;
                changed = true;
            }
        }
        if changed {
            self.save(all);
        }
        changed
    }

    /// Mark every notification read; persists.
    pub fn mark_all_read(&self) {
        let mut all = self.load();
        for n in all.iter_mut() {
            n.read = true;
        }
        self.save(all);
    }

    /// Remove a single notification. Returns `true` if one was removed; persists
    /// on removal.
    pub fn remove(&self, id: &str) -> bool {
        let mut all = self.load();
        let before = all.len();
        all.retain(|n| n.id != id);
        let removed = all.len() != before;
        if removed {
            self.save(all);
        }
        removed
    }

    /// Remove all notifications (factory reset); persists.
    pub fn clear_all(&self) {
        self.save(Vec::new());
    }

    // ── Persistence ─────────────────────────────────────────────────────────

    fn load(&self) -> Vec<AppNotification> {
        let mut guard = self.cache.lock();
        if let Some(ref c) = *guard {
            return c.clone();
        }
        let mut loaded: Vec<AppNotification> = std::fs::read_to_string(&self.data_file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        // Defensive cap: a hand-edited file could exceed the cap. The list is
        // persisted newest-first, so truncation drops the oldest.
        loaded.truncate(MAX_NOTIFICATIONS);
        *guard = Some(loaded.clone());
        loaded
    }

    fn save(&self, notifications: Vec<AppNotification>) {
        // Infallible (mirrors every sibling store), but a failed serialize or
        // write is logged rather than silently dropped so disk-persistence
        // problems are diagnosable. The in-memory cache is still updated so the
        // running session stays consistent even if the disk write failed.
        match serde_json::to_string_pretty(&notifications) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&self.data_file, &json) {
                    log::warn!("failed to persist notifications to disk: {e}");
                }
            }
            Err(e) => log::warn!("failed to serialize notifications: {e}"),
        }
        *self.cache.lock() = Some(notifications);
    }
}

// ── Factory-reset wiring ─────────────────────────────────────────────────────

/// Manage the notification store and register its factory-reset wipe in one
/// call, mirroring [`crate::extension_bridge::manage`]. After this, the store is
/// resolvable via `app.state::<NotificationStore>()` and wiped on full reset.
pub fn manage(
    app: &tauri::App,
    registry: &mut crate::commands::privacy::ResetRegistry,
    data_dir: &Path,
) {
    crate::commands::privacy::manage_resettable(
        app,
        registry,
        "notifications",
        NotificationStore::new(data_dir),
    );
}

#[cfg(test)]
mod tests;
