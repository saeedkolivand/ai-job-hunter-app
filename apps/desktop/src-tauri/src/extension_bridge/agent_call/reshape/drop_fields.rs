//! Dropping a dead/never-populated field from a reply — the wire-shape twin of `base64.rs`'s
//! re-encoding, but subtracting a key instead.

use serde_json::Value;

/// `(command, field)` pairs whose reply carries a field this codebase never gives a real value —
/// the wire-shape twin of [`BASE64_BYTE_FIELDS`] above, subtracting a key instead of re-encoding
/// one. Scoped to the agent-cli surface only: the renderer's own wire shape is untouched.
///
/// Audited: `autopilot_list`/`autopilot_get` → `autopilot::Autopilot.total_applied` (issue #1171's
/// residual) — `automations` already drops this from its own curated projection, but these two
/// commands dispatch RAW and return `totalApplied: 0` verbatim, contradicting `best-matches`'s
/// real `applied: true` on jobs it never counted. The struct field itself stays for now —
/// `docs/ARCHITECTURE_STATUS.md` tracks removing it everywhere, including the shared TS type.
pub(in crate::extension_bridge::agent_call) const DROP_FIELDS: &[(&str, &str)] = &[
    ("autopilot_list", "totalApplied"),
    ("autopilot_get", "totalApplied"),
];

/// Removes every [`DROP_FIELDS`] key from `command`'s reply — from a single
/// top-level object (`autopilot_get`) or from every element of a top-level
/// array (`autopilot_list`). A `null`/non-object/non-array reply (e.g.
/// `autopilot_get` on an unknown id) is left untouched — there is no field to
/// drop.
pub(in crate::extension_bridge::agent_call) fn drop_dead_fields(command: &str, data: &mut Value) {
    for (cmd, field) in DROP_FIELDS {
        if *cmd != command {
            continue;
        }
        match data {
            Value::Object(map) => {
                map.remove(*field);
            }
            Value::Array(items) => {
                for item in items.iter_mut() {
                    if let Value::Object(map) = item {
                        map.remove(*field);
                    }
                }
            }
            _ => {}
        }
    }
}
