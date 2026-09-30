//! The shared `list_models` result-projection helpers. Split out of `mod.rs`
//! (R8 line-budget split).

use serde_json::{json, Value};

/// Build one `list_models` entry: `{name, displayName?, createdAt?,
/// contextLength?}`. `name` is the canonical id everything selects on — a
/// stored model preference matches against it, so its shape/value must never
/// change here. Every other field is `None`-able because no single provider
/// endpoint returns all of them (see each adapter's `list_models`/
/// `parse_model_page` for exactly which it supplies) — a provider that omits
/// a field passes `None`, which is skipped entirely from the JSON, never a
/// fabricated zero/empty-string/"unknown" sentinel. The renderer treats
/// absent as absent.
///
/// `created_at_ms` is unix epoch MILLISECONDS — the SAME convention every
/// other timestamp field in this codebase already uses (`captured_at`,
/// `last_updated`, …: `chrono::Utc::now().timestamp_millis()`), not any
/// provider's native wire format (Anthropic ships an RFC3339 string, OpenAI a
/// unix-epoch-SECONDS integer, Ollama an RFC3339-with-offset string) — see
/// [`parse_rfc3339_millis`] for the RFC3339 → millis half of that
/// normalization. Chosen over keeping each provider's native representation
/// so the renderer sorts numerically with zero per-provider branching.
pub fn model_entry(
    name: &str,
    display_name: Option<&str>,
    created_at_ms: Option<i64>,
    context_length: Option<i64>,
) -> Value {
    let mut entry = json!({ "name": name });
    if let Some(d) = display_name {
        entry["displayName"] = json!(d);
    }
    if let Some(c) = created_at_ms {
        entry["createdAt"] = json!(c);
    }
    if let Some(l) = context_length {
        entry["contextLength"] = json!(l);
    }
    entry
}

/// Parse an RFC3339 timestamp (Anthropic's `created_at`, Ollama's
/// `modified_at` — both may carry a non-UTC offset, e.g. Ollama's
/// `-07:00`) into unix epoch milliseconds. `None` on any parse failure —
/// never a fabricated/zero timestamp; a parse failure is treated exactly
/// like the field being absent.
pub fn parse_rfc3339_millis(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp_millis())
}
