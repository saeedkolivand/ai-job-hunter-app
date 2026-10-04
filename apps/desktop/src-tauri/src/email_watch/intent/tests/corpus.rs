use super::*;

// ── corpus shape: pins the count against silent drift ──────────────────

#[test]
fn corpus_shape_matches_the_173_phrase_survived_corpus() {
    // Reads `lang` straight out of the raw JSON (not via `PhraseEntry`,
    // which doesn't map that key — see its doc) so a future edit to
    // `intent_phrases.json` can't silently drop entries unnoticed.
    let raw: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/email_watch/intent_phrases.json"
    )))
    .expect("valid JSON");
    let entries = raw.as_array().expect("top-level JSON array");
    assert_eq!(entries.len(), 173);
    assert_eq!(PHRASES.len(), 173);
    let per_lang = |lang: &str| entries.iter().filter(|e| e["lang"] == lang).count();
    assert_eq!(per_lang("en"), 30);
    assert_eq!(per_lang("de"), 27);
    assert_eq!(per_lang("fr"), 22);
    assert_eq!(per_lang("es"), 30);
    assert_eq!(per_lang("it"), 19);
    assert_eq!(per_lang("nl"), 17);
    assert_eq!(per_lang("pt"), 28);
}

#[test]
fn corpus_shape_pins_discriminating_counts_per_intent() {
    // `discriminating` is the SINGLE field gating whether a phrase can
    // decide an intent alone (see `PhraseEntry::discriminating`'s doc)
    // — flipping it on any one of ~114 non-discriminating entries would
    // silently change classifier behavior without touching a phrase's
    // text, its location, or any per-language total the previous test
    // already pins. Counted from `PHRASES` (post-parse, post-`fold`),
    // so this also exercises that the field actually deserializes.
    let count = |intent: EmailIntent| {
        PHRASES
            .iter()
            .filter(|p| p.intent == intent && p.discriminating)
            .count()
    };
    assert_eq!(count(EmailIntent::Confirmation), 9);
    assert_eq!(count(EmailIntent::Rejection), 76);
    assert_eq!(count(EmailIntent::Interview), 30);
    assert_eq!(count(EmailIntent::Offer), 23);
    assert_eq!(
        PHRASES.iter().filter(|p| p.discriminating).count(),
        9 + 76 + 30 + 23,
        "must equal the 138 total discriminating entries"
    );
}

#[test]
fn corpus_content_every_phrase_is_non_empty_and_already_lowercase() {
    for entry in PHRASES.iter() {
        assert!(
            !entry.phrase.trim().is_empty(),
            "an empty phrase can never usefully match anything — must be a data bug"
        );
        assert_eq!(
            entry.phrase,
            entry.phrase.to_lowercase(),
            "phrase {:?} is not already lower-cased — matching relies on this",
            entry.phrase
        );
    }
}

#[test]
fn corpus_content_every_discriminating_phrase_is_at_least_10_bytes() {
    // A short discriminating phrase can fire on ordinary, unrelated
    // mail — and per the write-path gating this classifier feeds, a
    // false rejection auto-writes an absorbing `Rejected`. 10 bytes is
    // the shortest phrase that survived the corpus's own adversarial
    // pass ("werturteil") — this pins that floor so a future addition
    // can't slip under it unnoticed.
    for entry in PHRASES.iter().filter(|p| p.discriminating) {
        assert!(
            entry.phrase.len() >= 10,
            "discriminating phrase {:?} is under 10 bytes — too short to \
                 safely decide an intent alone",
            entry.phrase
        );
    }
}
