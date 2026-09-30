//! A shared serde helper for the aggregator's loosely-typed fallback tiers
//! (Jooble, Apify) — split out of `providers.rs` (R8 module-size guard).
use serde::Deserialize;

// ── Serde helpers ─────────────────────────────────────────────────────────────

/// Like `adzuna::de_string_or_number` but for an OPTIONAL field, tolerating `null` /
/// absent / string / number. Used for the Apify actor's loosely-typed `id` and
/// `postedAt`, which vary by run (string id, numeric id, or omitted).
pub(super) fn de_opt_string_or_number<'de, D>(de: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(de)?;
    Ok(value.and_then(|v| match v {
        serde_json::Value::String(s) => Some(s),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }))
}
