//! Optional, local-only job-ad translation.
//!
//! When a job ad's detected language differs from the resume locale and the
//! user's ACTIVE AI provider (`ai_config::AiConfigStore::active_config` — the
//! SAME provider chat/document generation reads, never a stand-in) is local,
//! the JD is translated before keyword extraction so ATS matching happens in
//! the resume language. Shared platform infrastructure on top of the
//! centralized provider layer: it routes through the same resolve() +
//! AiProvider::complete() path as every other completion, so no
//! provider-specific assumption leaks here. Always the user's own active
//! model — never an arbitrary pick (see [`resolve_translation_target`]).
//!
//! Guardrails, all of which fall back to the original text (never an error):
//! cloud providers are excluded (ProviderId::is_local() gate) so translation
//! never incurs an unexpected API cost; uncertain detection, an unmapped
//! language, same-language, no active provider, no model configured for a
//! non-CLI-agent provider, or any LLM failure all return the original text.
//! Results are cached in-memory for the process, keyed by job id AND a hash
//! of the source text (see [`TranslationCache`]), under a size cap.

use std::collections::HashMap;
use std::sync::Mutex;

use tauri::{AppHandle, Manager};
use whatlang::Lang;

use super::ai_provider::{ollama, resolve, ProviderId};
use crate::documents::sha256_hex;

/// Process-scoped translation cache, keyed by job id AND source text. Managed
/// Tauri state.
pub struct TranslationCache(Mutex<HashMap<String, String>>);

/// Live-entry cap. Each value is a whole translated job ad (multi-KB) and this
/// map lives as long as the process — an assumption shaped by a UI session,
/// which the headless scheduler invalidated: it now feeds the cache up to
/// `SEMANTIC_RERANK_MAX` postings per run, every run, forever. On overflow the
/// map is dropped wholesale rather than evicted selectively — a `HashMap` has
/// no recency order to evict by, and the only cost of a miss is one local,
/// cloud-excluded re-translation.
const MAX_CACHE_ENTRIES: usize = 256;

/// The cache identity of one translation.
///
/// `job_id` ALONE is not an identity. A board edits a description in place and
/// the id does not move with it, so run 1's translation is served for run 2's
/// text — and the score cannot catch it either: `match_scores` keys on a hash of
/// the POST-translation text, so a stale translation re-hashes to the stale key
/// and its old row is served with it. Autopilot's `autopilot:<hash>` ids are
/// stable across runs by design, so this is the ordinary path there, not an edge
/// case. The text is what was translated, so the text is in the key.
fn cache_key(job_id: &str, source_text: &str) -> String {
    format!("{job_id}\u{1f}{}", sha256_hex(source_text))
}

impl Default for TranslationCache {
    fn default() -> Self {
        Self::new()
    }
}

impl TranslationCache {
    pub fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    pub fn get(&self, job_id: &str, source_text: &str) -> Option<String> {
        self.0
            .lock()
            .ok()?
            .get(&cache_key(job_id, source_text))
            .cloned()
    }

    pub fn set(&self, job_id: &str, source_text: &str, translated: String) {
        if let Ok(mut m) = self.0.lock() {
            if m.len() >= MAX_CACHE_ENTRIES {
                m.clear();
            }
            m.insert(cache_key(job_id, source_text), translated);
        }
    }
}

/// Translate `text` into `target_lang` only when the detected source language
/// differs from the target AND a local provider is configured.
///
/// `target_lang` is a BCP-47 tag ("en", "de", "fr", ...). Falls back to the
/// original `text` on any uncertainty or failure: low-confidence detection,
/// unmapped language, same language, a non-local active provider, no reachable
/// local chat model, or an LLM error. Successful translations are cached under
/// `(job_id, hash(text))`, so an edited description re-translates instead of
/// serving the previous run's text.
pub async fn translate_if_needed(
    app: &AppHandle,
    job_id: &str,
    text: &str,
    target_lang: &str,
) -> String {
    // 0. A cached translation OF THIS EXACT TEXT wins immediately.
    if let Some(cache) = app.try_state::<TranslationCache>() {
        if let Some(cached) = cache.get(job_id, text) {
            return cached;
        }
    }

    // 1. Detect the source language. Bail on uncertain detection.
    let Some(info) = whatlang::detect(text) else {
        return text.to_string();
    };
    if !info.is_reliable() {
        return text.to_string();
    }
    let Some(source_bcp47) = lang_to_bcp47(info.lang()) else {
        return text.to_string();
    };
    // 2. Already in the target language: nothing to do.
    if source_bcp47.eq_ignore_ascii_case(target_lang) {
        return text.to_string();
    }

    // 3-4. Resolve the provider + model from the user's ACTIVE AI config —
    //    the SAME store chat/document generation reads
    //    (`ai_config::AiConfigStore::active_config`), never a stand-in. Cloud
    //    providers are excluded so translation can never incur an unexpected
    //    API cost, even though autopilot can call this on up to
    //    `SEMANTIC_RERANK_MAX` (20) postings per scheduled run, indefinitely.
    //    Always the user's OWN active model — never an arbitrary pick (see
    //    `resolve_translation_target`'s doc comment for why that matters).
    let Some(ai_config) = app.try_state::<crate::ai_config::AiConfigStore>() else {
        return text.to_string();
    };
    let cfg = ai_config.active_config();
    let Some((provider_id, model)) =
        resolve_translation_target(cfg.active_provider.as_deref(), cfg.model.as_deref())
    else {
        return text.to_string();
    };

    // Ollama specifically: a fast reachability probe before the completion
    // call — the same HEALTH-timeout (3s) check the now-deleted
    // `reachable_chat_model` used to gate on before this function switched
    // from the embedding config to the active-provider config. Without it, an
    // unreachable OR merely slow/busy local daemon eats the full completion
    // deadline (minutes, scaled by effort — see `timeouts::OLLAMA_COMPLETION_BASELINE`)
    // per translation attempt instead of a fast skip, which is exactly the
    // run-starving shape this file's own module doc + this fix's own commit
    // message describe. A CLI agent has no analogous cheap health check and
    // is not gated here — its own `.complete()` call fails fast when the
    // binary is missing or unauthenticated.
    if provider_id == ProviderId::Ollama
        && !should_attempt_translation(provider_id, ollama::reachable_model().await.0)
    {
        return text.to_string();
    }

    // 5. Translate through the centralized provider layer.
    let target_display = lang_display(target_lang);
    let system = format!(
        "Translate the following text to {target_display}. Keep all technical terms, \
         programming languages, framework names, and proper nouns exactly as written. \
         Return only the translated text, no explanations."
    );
    let provider = resolve(provider_id, cfg.base_url);
    match provider
        .complete(app, &model, &system, text, Some(0.1))
        .await
    {
        Ok(translated) if !translated.trim().is_empty() => {
            if let Some(cache) = app.try_state::<TranslationCache>() {
                cache.set(job_id, text, translated.clone());
            }
            translated
        }
        // Empty or errored translation: safe fallback to the original text — but
        // NOT a silent one. This degrading quietly is how a local setup with only
        // an embedding model installed sent that model to `/api/chat` and took a
        // 400 per job ad for a whole session with nothing but an `ok=false` span
        // to show for it.
        Ok(_) => {
            tracing::warn!(model = %model, "translation: provider returned empty text, keeping the original");
            text.to_string()
        }
        Err(e) => {
            tracing::warn!(model = %model, "translation failed, keeping the original: {e}");
            text.to_string()
        }
    }
}

/// Resolve `(provider_id, model)` for a translation request from the active
/// AI config — `None` means skip translation and fall back to the original
/// text: no active provider, a metered (non-local) provider, or a model
/// [`ProviderId::validate_model`] rejects (empty on anything but a CLI
/// agent, which validly falls back to the tool's own default). Pure, so it
/// is directly unit-testable without an `AppHandle` — extracted for the same
/// reason `pipeline::Completer::resolve_parts` is.
///
/// Always the user's OWN active model — never an arbitrary pick. A previous
/// version of this path picked whatever chat model Ollama's `/api/tags`
/// happened to list first, which once sent a 27B model to translate one job
/// ad: 117 seconds, starving the concurrent embedding calls' 15s timeout for
/// the whole run.
fn resolve_translation_target(
    active_provider: Option<&str>,
    model: Option<&str>,
) -> Option<(ProviderId, String)> {
    let active_provider = active_provider?;
    if !provider_allows_translation(active_provider) {
        return None;
    }
    let provider_id = ProviderId::parse(active_provider).ok()?;
    let model = model.unwrap_or_default().to_string();
    provider_id.validate_model(&model).ok()?;
    Some((provider_id, model))
}

/// Whether `translate_if_needed` should proceed to the completion call, given
/// whether Ollama's fast reachability probe (if it ran) reported the daemon
/// reachable. Only [`ProviderId::Ollama`] is gated by `ollama_reachable` — a
/// CLI agent has no analogous cheap health check, so it always proceeds and
/// relies on its own `.complete()` call failing fast when the binary is
/// missing or unauthenticated. Pure, so the gate is unit-testable without a
/// live Ollama daemon.
fn should_attempt_translation(provider_id: ProviderId, ollama_reachable: bool) -> bool {
    provider_id != ProviderId::Ollama || ollama_reachable
}

/// Map a `whatlang::Lang` to a BCP-47 tag for the languages we translate
/// between. `None` for anything else, which skips translation entirely.
fn lang_to_bcp47(lang: Lang) -> Option<&'static str> {
    Some(match lang {
        Lang::Eng => "en",
        Lang::Deu => "de",
        Lang::Fra => "fr",
        Lang::Spa => "es",
        Lang::Ita => "it",
        Lang::Por => "pt",
        Lang::Nld => "nl",
        Lang::Pol => "pl",
        Lang::Rus => "ru",
        Lang::Cmn => "zh", // whatlang models Chinese as Mandarin (Cmn)
        Lang::Jpn => "ja",
        Lang::Kor => "ko",
        _ => return None,
    })
}

/// Human-readable language name for the translation prompt. Unknown tags fall
/// back to English so the prompt is always well-formed.
fn lang_display(bcp47: &str) -> &'static str {
    match bcp47.to_ascii_lowercase().as_str() {
        "en" => "English",
        "de" => "German",
        "fr" => "French",
        "es" => "Spanish",
        "it" => "Italian",
        "pt" => "Portuguese",
        "nl" => "Dutch",
        "pl" => "Polish",
        "ru" => "Russian",
        "zh" => "Chinese",
        "ja" => "Japanese",
        "ko" => "Korean",
        _ => "English",
    }
}

/// Returns `true` only when `s` parses as a local provider (Ollama or a CLI
/// agent such as ClaudeCode / Codex / GeminiCli). Cloud providers and any
/// unrecognised string return `false`, keeping translation gated to local
/// inference only and never incurring unexpected API costs.
pub(crate) fn provider_allows_translation(s: &str) -> bool {
    ProviderId::parse(s)
        .map(|id| id.is_local())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests;
