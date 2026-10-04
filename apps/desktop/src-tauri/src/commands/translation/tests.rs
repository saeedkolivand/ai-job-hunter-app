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
