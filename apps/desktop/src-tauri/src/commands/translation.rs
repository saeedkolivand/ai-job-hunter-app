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
mod tests {
    use super::*;

    const DE: &str = "Eine Stellenbeschreibung.";

    #[test]
    fn cache_round_trips() {
        let cache = TranslationCache::new();
        assert!(cache.get("job-1", DE).is_none());
        cache.set("job-1", DE, "translated".to_string());
        assert_eq!(cache.get("job-1", DE).as_deref(), Some("translated"));
        // Distinct keys are isolated.
        assert!(cache.get("job-2", DE).is_none());
    }

    #[test]
    fn cache_set_overwrites() {
        let cache = TranslationCache::new();
        cache.set("j", DE, "first".to_string());
        cache.set("j", DE, "second".to_string());
        assert_eq!(cache.get("j", DE).as_deref(), Some("second"));
    }

    /// The cache used exactly as [`translate_if_needed`] uses it — look up,
    /// then on a miss translate and store — with only the LLM replaced by a
    /// counter. The key, the map and the call ordering are all the real ones.
    fn memoized_translate(
        cache: &TranslationCache,
        calls: &std::cell::Cell<usize>,
        job_id: &str,
        text: &str,
    ) -> String {
        if let Some(hit) = cache.get(job_id, text) {
            return hit;
        }
        calls.set(calls.get() + 1);
        let translated = format!("EN[{text}]");
        cache.set(job_id, text, translated.clone());
        translated
    }

    /// A job id is not a translation identity: the board can edit a description
    /// in place under the same id. Autopilot makes that the ORDINARY path —
    /// `autopilot:<sha256(canonical_job_key)>` is deliberately stable across
    /// runs, so a job-id-only key pins run 1's translation for the life of the
    /// process. And nothing downstream can catch it: `match_scores` keys on a
    /// hash of the POST-translation text, so the stale translation re-hashes to
    /// the stale key and serves its old score row along with it.
    #[test]
    fn a_changed_description_under_the_same_job_id_is_a_cache_miss() {
        let cache = TranslationCache::new();
        let calls = std::cell::Cell::new(0);
        let job_id = "autopilot:stable-across-runs";
        let original = "Wir suchen einen Entwickler mit Rust-Erfahrung.";
        let edited = "Wir suchen einen Entwickler mit Rust- und Kubernetes-Erfahrung.";

        // Run 1 translates the description the board published.
        let first = memoized_translate(&cache, &calls, job_id, original);
        assert_eq!(calls.get(), 1);

        // Unchanged text must still HIT — the cache has to keep doing its job,
        // or this fix would just be a disabled cache.
        assert_eq!(memoized_translate(&cache, &calls, job_id, original), first);
        assert_eq!(
            calls.get(),
            1,
            "identical source text under the same id is still one translation"
        );

        // Run 2: the board edited the description; the id did not move.
        let second = memoized_translate(&cache, &calls, job_id, edited);
        assert_eq!(
            calls.get(),
            2,
            "changed source text must MISS — otherwise run 1's translation is \
             served forever, and the match_scores row keyed on its hash with it"
        );
        assert_ne!(second, first);
        assert!(
            second.contains("Kubernetes"),
            "the NEW text must be what was translated: {second}"
        );
    }

    /// The map is process-scoped and the headless scheduler feeds it every run,
    /// so it needs a ceiling. Asserted behaviourally (the oldest entry is gone
    /// after overflow) rather than through a test-only `len()` accessor.
    #[test]
    fn the_cache_does_not_grow_without_bound() {
        let cache = TranslationCache::new();
        cache.set("job", "the very first description", "first".to_string());
        for i in 0..MAX_CACHE_ENTRIES {
            cache.set("job", &format!("filler description {i}"), "x".to_string());
        }
        assert!(
            cache.get("job", "the very first description").is_none(),
            "a process-lifetime cache of whole job ads must be bounded"
        );
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

    // ── resolve_translation_target (item 1+2: active provider, own model) ──────

    #[test]
    fn claude_code_active_provider_uses_its_own_active_model() {
        assert_eq!(
            resolve_translation_target(Some("claude-code"), Some("opus")),
            Some((ProviderId::ClaudeCode, "opus".to_string()))
        );
    }

    #[test]
    fn claude_code_with_no_configured_model_falls_back_to_the_tools_own_default() {
        // CLI agents validly run with an empty model (the tool's own default) —
        // `validate_model` allows it, so translation must still proceed rather
        // than skip.
        assert_eq!(
            resolve_translation_target(Some("claude-code"), None),
            Some((ProviderId::ClaudeCode, String::new()))
        );
    }

    #[test]
    fn ollama_uses_the_configured_model_never_an_arbitrary_one() {
        // Two distinct configured models must echo back UNCHANGED — proof this
        // is never a hardcoded or "first installed" pick, only ever what the
        // caller passed in as the active model.
        assert_eq!(
            resolve_translation_target(Some("ollama"), Some("qwen3.6:27b-q4_K_M")),
            Some((ProviderId::Ollama, "qwen3.6:27b-q4_K_M".to_string()))
        );
        assert_eq!(
            resolve_translation_target(Some("ollama"), Some("gemma4:9b")),
            Some((ProviderId::Ollama, "gemma4:9b".to_string()))
        );
    }

    #[test]
    fn ollama_with_no_configured_model_skips_translation() {
        // Unlike a CLI agent, Ollama has no "tool's own default" to fall back
        // to — an unconfigured model must skip, never silently pick one.
        assert_eq!(resolve_translation_target(Some("ollama"), None), None);
    }

    #[test]
    fn metered_providers_skip_translation_regardless_of_model() {
        for provider in [
            "openai",
            "anthropic",
            "gemini",
            "ollama-cloud",
            "openai-compatible",
        ] {
            assert_eq!(
                resolve_translation_target(Some(provider), Some("some-model")),
                None,
                "{provider} must never receive a translation call"
            );
        }
    }

    #[test]
    fn no_active_provider_skips_translation() {
        assert_eq!(resolve_translation_target(None, Some("opus")), None);
    }

    #[test]
    fn unknown_provider_string_skips_translation() {
        assert_eq!(
            resolve_translation_target(Some("unknown-provider"), Some("x")),
            None
        );
    }

    // ── should_attempt_translation (Ollama reachability gate) ──────────────

    #[test]
    fn an_unreachable_ollama_daemon_is_gated_before_the_completion_call() {
        assert!(!should_attempt_translation(ProviderId::Ollama, false));
        assert!(should_attempt_translation(ProviderId::Ollama, true));
    }

    #[test]
    fn a_cli_agent_has_no_reachability_gate() {
        // CLI agents have no cheap health check, so `ollama_reachable` (always
        // `false` here — the caller never runs the probe for a non-Ollama
        // provider) must not skip them.
        for provider in [
            ProviderId::ClaudeCode,
            ProviderId::Codex,
            ProviderId::GeminiCli,
            ProviderId::Antigravity,
        ] {
            assert!(
                should_attempt_translation(provider, false),
                "{provider:?} must proceed regardless of the (irrelevant) Ollama probe"
            );
        }
    }
}
