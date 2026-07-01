//! Optional, local-only job-ad translation.
//!
//! When a job ad's detected language differs from the resume locale and a
//! local provider (Ollama) is configured, the JD is translated before keyword
//! extraction so ATS matching happens in the resume language. Shared platform
//! infrastructure on top of the centralized provider layer: it routes through
//! the same resolve() + AiProvider::complete() path as every other completion,
//! so no provider-specific assumption leaks here.
//!
//! Guardrails, all of which fall back to the original text (never an error):
//! cloud providers are excluded (ProviderId::is_local() gate) so translation
//! never incurs an unexpected API cost; uncertain detection, an unmapped
//! language, same-language, no reachable local model, or any LLM failure all
//! return the original text. Results are cached in-memory per session by job_id.

use std::collections::HashMap;
use std::sync::Mutex;

use tauri::{AppHandle, Manager};
use whatlang::Lang;

use super::ai_provider::{resolve, ProviderId};
use crate::documents::DocumentStore;

/// Session-scoped translation cache, keyed by job_id. Managed Tauri state.
pub struct TranslationCache(Mutex<HashMap<String, String>>);

impl Default for TranslationCache {
    fn default() -> Self {
        Self::new()
    }
}

impl TranslationCache {
    pub fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    pub fn get(&self, job_id: &str) -> Option<String> {
        self.0.lock().ok()?.get(job_id).cloned()
    }

    pub fn set(&self, job_id: String, text: String) {
        if let Ok(mut m) = self.0.lock() {
            m.insert(job_id, text);
        }
    }
}

/// Translate `text` into `target_lang` only when the detected source language
/// differs from the target AND a local provider is configured.
///
/// `target_lang` is a BCP-47 tag ("en", "de", "fr", ...). Falls back to the
/// original `text` on any uncertainty or failure: low-confidence detection,
/// unmapped language, same language, a non-local active provider, no reachable
/// local chat model, or an LLM error. Successful translations are cached by
/// `job_id` for the session.
pub async fn translate_if_needed(
    app: &AppHandle,
    job_id: &str,
    text: &str,
    target_lang: &str,
) -> String {
    // 0. Cached translation for this job wins immediately.
    if let Some(cache) = app.try_state::<TranslationCache>() {
        if let Some(cached) = cache.get(job_id) {
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

    // 3. Active provider must be LOCAL. Cloud providers are excluded so
    //    translation can never incur an unexpected API cost. The server-side
    //    active provider is the persisted embedding config (defaults to
    //    Ollama): the only provider/model selection available without a
    //    request-supplied provider.
    let Some(store) = app.try_state::<DocumentStore>() else {
        return text.to_string();
    };
    let cfg = store.embedding_config();
    if !provider_allows_translation(&cfg.provider) {
        return text.to_string();
    }

    // 4. Resolve a reachable local chat model via the provider layer.
    //    Re-parse is infallible: provider_allows_translation confirmed it parses.
    let Ok(provider_id) = ProviderId::parse(&cfg.provider) else {
        return text.to_string();
    };
    let Some(model) = super::ai_provider::reachable_chat_model(provider_id).await else {
        return text.to_string();
    };

    // 5. Translate through the centralized provider layer.
    let target_display = lang_display(target_lang);
    let system = format!(
        "Translate the following text to {target_display}. Keep all technical terms, \
         programming languages, framework names, and proper nouns exactly as written. \
         Return only the translated text, no explanations."
    );
    let provider = resolve(provider_id, cfg.base_url.clone());
    match provider
        .complete(app, &model, &system, text, Some(0.1))
        .await
    {
        Ok(translated) if !translated.trim().is_empty() => {
            if let Some(cache) = app.try_state::<TranslationCache>() {
                cache.set(job_id.to_string(), translated.clone());
            }
            translated
        }
        // Empty or errored translation: safe fallback to the original text.
        _ => text.to_string(),
    }
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
mod tests {
    use super::*;

    #[test]
    fn cache_round_trips() {
        let cache = TranslationCache::new();
        assert!(cache.get("job-1").is_none());
        cache.set("job-1".to_string(), "translated".to_string());
        assert_eq!(cache.get("job-1").as_deref(), Some("translated"));
        // Distinct keys are isolated.
        assert!(cache.get("job-2").is_none());
    }

    #[test]
    fn cache_set_overwrites() {
        let cache = TranslationCache::new();
        cache.set("j".to_string(), "first".to_string());
        cache.set("j".to_string(), "second".to_string());
        assert_eq!(cache.get("j").as_deref(), Some("second"));
    }

    #[test]
    fn covered_langs_map_to_expected_bcp47() {
        assert_eq!(lang_to_bcp47(Lang::Eng), Some("en"));
        assert_eq!(lang_to_bcp47(Lang::Deu), Some("de"));
        assert_eq!(lang_to_bcp47(Lang::Fra), Some("fr"));
        assert_eq!(lang_to_bcp47(Lang::Spa), Some("es"));
        assert_eq!(lang_to_bcp47(Lang::Ita), Some("it"));
        assert_eq!(lang_to_bcp47(Lang::Por), Some("pt"));
        assert_eq!(lang_to_bcp47(Lang::Nld), Some("nl"));
        assert_eq!(lang_to_bcp47(Lang::Pol), Some("pl"));
        assert_eq!(lang_to_bcp47(Lang::Rus), Some("ru"));
        assert_eq!(lang_to_bcp47(Lang::Cmn), Some("zh"));
        assert_eq!(lang_to_bcp47(Lang::Jpn), Some("ja"));
        assert_eq!(lang_to_bcp47(Lang::Kor), Some("ko"));
    }

    #[test]
    fn uncovered_lang_returns_none() {
        // A language outside the covered set skips translation.
        assert_eq!(lang_to_bcp47(Lang::Tur), None);
    }

    #[test]
    fn display_names_for_each_tag() {
        assert_eq!(lang_display("de"), "German");
        assert_eq!(lang_display("EN"), "English"); // case-insensitive
        assert_eq!(lang_display("zh"), "Chinese");
        assert_eq!(lang_display("ja"), "Japanese");
        assert_eq!(lang_display("ko"), "Korean");
        // Unknown tag is well-formed (defaults to English).
        assert_eq!(lang_display("xx"), "English");
    }

    #[test]
    fn local_providers_allow_translation() {
        assert!(provider_allows_translation("ollama"));
        assert!(provider_allows_translation("claude-code")); // CLI agent → is_local()
        assert!(provider_allows_translation("codex")); // CLI agent → is_local()
        assert!(provider_allows_translation("gemini-cli")); // CLI agent → is_local()
    }

    #[test]
    fn cloud_providers_block_translation() {
        assert!(!provider_allows_translation("openai"));
        assert!(!provider_allows_translation("anthropic"));
        assert!(!provider_allows_translation("gemini"));
        assert!(!provider_allows_translation("ollama-cloud")); // paid cloud Ollama
        assert!(!provider_allows_translation("openai-compatible"));
    }

    #[test]
    fn unknown_provider_blocks_translation() {
        // parse() returns Err → unwrap_or(false)
        assert!(!provider_allows_translation("unknown-provider"));
        assert!(!provider_allows_translation(""));
    }

    #[test]
    fn bcp47_and_display_round_trip_agrees() {
        // Ensures lang_to_bcp47 and lang_display are consistent with each other.
        // A swap in both tables simultaneously (e.g. "ja"↔"ko") would still fail here.
        assert_eq!(lang_display(lang_to_bcp47(Lang::Jpn).unwrap()), "Japanese");
        assert_eq!(lang_display(lang_to_bcp47(Lang::Kor).unwrap()), "Korean");
        assert_eq!(lang_display(lang_to_bcp47(Lang::Deu).unwrap()), "German");
        assert_ne!(
            lang_display(lang_to_bcp47(Lang::Jpn).unwrap()),
            lang_display(lang_to_bcp47(Lang::Kor).unwrap()),
            "Japanese and Korean BCP-47 tags must not be swapped"
        );
    }
}
