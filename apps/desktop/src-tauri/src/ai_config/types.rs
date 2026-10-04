//! The shapes [`AiConfigStore`](super::AiConfigStore) reads and writes: one
//! provider's persisted settings, the export/import snapshot (parsed leniently
//! from untrusted input), the read model and the PATCH the settings writer takes.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

use super::stage_overrides::StageOverride;
use crate::error::AppResult;

/// One provider's persisted generation settings. `base_url` is only meaningful
/// for `openai-compatible`; `model` is empty/absent for a not-yet-configured
/// provider (and legitimately empty for CLI agents, which use their own default).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// The context window (`num_ctx`) to run **`model`** with, when the user
    /// configured one. Belongs to the model in THIS row, not to the provider:
    /// the renderer's own limits map is keyed by model, so the two move
    /// together — `set_provider_settings` replaces both or neither, and a row
    /// whose model changed without a new window is a row with no window.
    ///
    /// Only Ollama reads it (`options.num_ctx`); every other adapter ignores
    /// it, which is why it is stored rather than gated per provider — a user
    /// who switches provider and back keeps the value they set.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient_context_window"
    )]
    pub context_window: Option<u32>,
}

/// The persisted snapshot — the export/import/seed shape (`{ activeProvider,
/// providers }`), 1:1 with the renderer's old Zustand `aiProviderConfig`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfigSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_provider: Option<String>,
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderConfig>,
    /// Per-stage model overrides, keyed by the generated stage vocabulary. A
    /// defaulted field so a bundle (or a first-run renderer seed) written
    /// before overrides existed still deserializes, and an empty map is
    /// omitted from the export entirely.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stage_overrides: BTreeMap<String, StageOverride>,
}

impl AiConfigSnapshot {
    /// Parse a RESTORE bundle's `aiProviderConfig` section entry by entry, so a
    /// single unparseable entry costs only itself.
    ///
    /// A plain `from_value::<AiConfigSnapshot>` is all-or-nothing, and that is
    /// the wrong shape for untrusted input: a probe of five hand-editable
    /// mistakes (`-1`, `2^32`, a missing `provider`, a non-object entry, a
    /// wrong-typed `model`) showed EVERY one aborting the whole section —
    /// active provider `None`, providers 0, overrides 0, with the valid rows in
    /// the same bundle lost too. Field-level leniency
    /// ([`lenient_context_window`], `StageOverride::provider`) fixes the rows
    /// whose FIELDS are recoverable; this fixes the ones whose SHAPE is not, by
    /// dropping the entry rather than its siblings.
    ///
    /// Only the section itself being unparseable is still an error: that is a
    /// corrupt bundle, not one bad row, and reporting it beats silently
    /// restoring nothing.
    pub(super) fn from_untrusted(data: &serde_json::Value) -> AppResult<Self> {
        let obj = data.as_object().ok_or_else(|| {
            crate::error::AppError::Parse(
                "the aiProviderConfig section is not an object".to_string(),
            )
        })?;
        // A wrong-typed `activeProvider` is dropped rather than fatal — the
        // store simply reads as unseeded, which is a state it already handles.
        let active_provider = obj
            .get("activeProvider")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        Ok(Self {
            active_provider,
            providers: parse_entries(obj.get("providers")),
            stage_overrides: parse_entries(obj.get("stageOverrides")),
        })
    }
}

/// One `{ key: entry }` map, parsed per entry — an entry that will not
/// deserialize is dropped, never the map. Shared by both of the snapshot's
/// maps because both take the same untrusted input.
fn parse_entries<T: serde::de::DeserializeOwned>(
    value: Option<&serde_json::Value>,
) -> BTreeMap<String, T> {
    value
        .and_then(serde_json::Value::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(k, v)| {
                    serde_json::from_value::<T>(v.clone())
                        .ok()
                        .map(|parsed| (k.clone(), parsed))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The read model returned to the renderer: the active provider's own resolved
/// `model`/`baseUrl` (the convenience `useGenerateConfig` reads) plus the full
/// `providers` map (for the Settings AI tab). `activeProvider`/`model`/`baseUrl`
/// are all absent when unseeded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveAiConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// The active provider row's [`ProviderConfig::context_window`] — the
    /// window `model` is configured to run with, or absent when the user never
    /// set one (in which case the provider keeps its own default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    pub providers: BTreeMap<String, ProviderConfig>,
}

/// A PATCH to one provider's settings — the shape the settings writer takes.
///
/// Per field: **absent = keep what is stored**, explicit `null` = clear, a value
/// = set. Replace-everything semantics were the first design and they failed on
/// first contact: three renderer call sites each saved one field and silently
/// erased the other two, and a doc comment saying "send them all" is not a
/// mechanism. Absence is what a caller produces by accident, so absence has to
/// be the harmless answer.
///
/// Hand-written rather than emitted by `pnpm gen:ipc`: the whole point is the
/// `Option<Option<T>>` + `deserialize_with` pair below, which the generator has
/// no way to express. The TS counterpart is
/// `AiContract.setProviderSettings` (`field?: T | null`) — keep the two in step
/// by hand, and prefer adding a field HERE first so the compiler catches the
/// store side.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettingsPatch {
    pub provider: String,
    #[serde(default, deserialize_with = "double_option")]
    pub model: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub base_url: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub context_window: Option<Option<u32>>,
}

/// Distinguish "the key was absent" (`None`) from "the key was present and
/// null" (`Some(None)`).
///
/// Needed because a plain `Option<Option<T>>` collapses both to `None`: serde's
/// `deserialize_option` visits `none` for a missing key AND for an explicit
/// null. This is also why the command takes a struct rather than loose
/// arguments — a Tauri command parameter has no serde attributes to hang this
/// on.
fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::deserialize(deserializer).map(Some)
}

/// Read a stored context window WITHOUT letting a bad one fail its row.
///
/// The derived `Option<u32>` rejects `-1` and `2^32` at PARSE time, which in a
/// restore is not a per-field failure at all: it aborts the whole
/// `aiProviderConfig` section, so a bundle with one hand-edited number restores
/// no providers, no active provider, and no overrides. That contradicts what
/// both scrub paths promise — an out-of-range value costs the row its window,
/// not its existence, and one bad entry never fails the restore wholesale.
///
/// So anything a `u32` cannot represent reads as `None` (= the provider's own
/// default) and the row survives to be validated normally. `Value` rather than
/// `i64` so a float, a string or a null is tolerated too — every shape, not
/// just the two that were reported.
///
/// REPRESENTABILITY only. Whether an in-range-looking number is actually
/// allowed stays with the scrub the apply path already runs
/// (`scrub_settings` / `apply_stage_overrides_conn`), so the bound lives in one
/// place: re-checking it here passed every test with the check deleted, which
/// is what a second owner of the same rule looks like. This is the LENIENT side
/// of the split `scrub_settings` makes — the interactive writer still errors,
/// because there a human typed the number and can be told it was refused.
pub(super) fn lenient_context_window<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(raw
        .as_ref()
        .and_then(serde_json::Value::as_u64)
        .and_then(|v| u32::try_from(v).ok()))
}
