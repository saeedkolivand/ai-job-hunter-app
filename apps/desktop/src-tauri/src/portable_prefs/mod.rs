//! Portable preferences: the allowlisted slice of the user's settings that
//! follows them to another device through the optional self-hosted sync
//! (ADR-051, #1433).
//!
//! The store holds ONLY the keys in [`PORTABLE_KEYS`], each with its own
//! `updatedAt`, so two devices merge per key (last writer wins) and a key the
//! user never set is simply absent: a fresh install can never push a default
//! over a value another device holds. Everything not listed is per-device
//! (performance, local model limits and paths, launch-at-login, close-to-tray),
//! per-device-with-consent (logo enrichment, indexing, extension permissions,
//! email watching) or a secret (API keys, pairing secrets, sessions): none of
//! it can be written here, exported, or applied from a received payload.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

mod store;

pub use self::store::PortablePrefsStore;

/// Bundle / sync section key.
pub const SECTION: &str = "portablePreferences";
/// The section's schema version: a device on an older app holds a newer
/// payload until it updates (`sync::rules`), it never down-converts.
pub const SCHEMA_VERSION: u32 = 1;

/// The only keys that may be stored or synced. Values are opaque JSON the
/// renderer validates against its own schema on read.
pub const PORTABLE_KEYS: &[&str] = &[
    "userName",
    "language",
    "outputTone",
    "resume",
    "applicant",
    "promptQuality",
    "providerEffort",
    "recentLocations",
    "theme",
];

/// A single value is a small setting; anything bigger is not one.
const MAX_VALUE_BYTES: usize = 16 * 1024;

/// One stored key: its value and when the user last changed it (ms).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub value: Value,
    pub updated_at: u64,
}

pub type Keys = BTreeMap<String, Entry>;

pub fn is_portable(key: &str) -> bool {
    PORTABLE_KEYS.contains(&key)
}

/// Whether `value` is an acceptable setting value (present and small).
pub fn value_ok(value: &Value) -> bool {
    !value.is_null() && serde_json::to_string(value).is_ok_and(|s| s.len() <= MAX_VALUE_BYTES)
}

/// The keys of a received or restored section. Unknown keys, malformed or
/// oversized entries are dropped one by one, never failing the whole payload.
pub fn parse_wire(payload: &Value) -> Keys {
    let Some(keys) = payload.get("keys").and_then(Value::as_object) else {
        return Keys::new();
    };
    keys.iter()
        .filter(|(k, _)| is_portable(k))
        .filter_map(|(k, e)| {
            let entry: Entry = serde_json::from_value(e.clone()).ok()?;
            value_ok(&entry.value).then(|| (k.clone(), entry))
        })
        .collect()
}

/// The section as it travels: a version and the keys.
pub fn to_wire(keys: &Keys) -> Value {
    json!({ "version": SCHEMA_VERSION, "keys": keys })
}

/// Merge `remote` into `local` per key: a remote entry wins only when it is
/// strictly newer (a tie keeps the local one). True when `local` changed.
pub fn merge(local: &mut Keys, remote: Keys) -> bool {
    let mut changed = false;
    for (key, entry) in remote {
        let newer = local
            .get(&key)
            .is_none_or(|mine| entry.updated_at > mine.updated_at);
        if newer {
            local.insert(key, entry);
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests;
