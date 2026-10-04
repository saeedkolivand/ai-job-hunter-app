use super::*;

// ── Lexical arm ──────────────────────────────────────────────────────────────

#[test]
fn the_question_is_the_title_column_and_the_answer_is_the_description() {
    let entries = corpus();
    let doc = to_lexical_doc(&entries[0]);
    assert_eq!(doc.id, "documentsQuestions.importFormats");
    assert_eq!(doc.title, entries[0].title, "the question must be `title`");
    assert_eq!(
        doc.description, entries[0].body,
        "the answer must be `description`"
    );
    assert_eq!(doc.company, "", "no help-corpus counterpart");
    assert_eq!(doc.location, "", "no help-corpus counterpart");
}

#[test]
fn a_query_matching_only_an_answers_wording_still_ranks_that_entry_first() {
    // "scanned" appears in exactly one entry, and only in its ANSWER — so
    // this can only pass if the body is indexed as a searchable column. It is
    // the direct check on the mapping `to_lexical_doc` chooses.
    let (ranks, status) = run_lexical_arm(&corpus(), "scanned", 3, Some("en"));
    assert_eq!(status, ArmStatus::Ran);
    assert_eq!(
        ranks.first().map(String::as_str),
        Some("documentsQuestions.importFormats"),
        "got {ranks:?}"
    );
}

#[test]
fn a_question_word_outranks_the_same_word_buried_in_another_answer() {
    // A dedicated fixture, because the property needs the term in exactly one
    // column per entry: "export" is ONLY in `buried`'s answer and ONLY in
    // `asked`'s question. BM25 weights title at 3.0 and description at 1.0
    // (`retrieval::lexical::BM25_WEIGHTS`), so the entry that ASKS about
    // exporting must win. Swap the two columns in `to_lexical_doc` and this
    // inverts.
    let entries = vec![
        entry(
            "buried",
            "Which file formats can I import?",
            "Drop a file onto Resume Management. You can export the result again afterwards.",
        ),
        entry(
            "asked",
            "How do I export a finished document?",
            "Press the button above a finished document and choose PDF, DOCX or TXT.",
        ),
    ];
    let (ranks, _) = run_lexical_arm(&entries, "export", 3, Some("en"));
    assert_eq!(
        ranks.len(),
        2,
        "both entries must MATCH, or this measures a lookup rather than a ranking: {ranks:?}"
    );
    assert_eq!(
        ranks.first().map(String::as_str),
        Some("asked"),
        "the question hit must outrank the answer hit; got {ranks:?}"
    );
}

#[test]
fn a_query_matching_nothing_is_an_empty_ran_arm_not_a_failure() {
    let (ranks, status) = run_lexical_arm(&corpus(), "kubernetes", 3, Some("en"));
    assert!(ranks.is_empty());
    assert_eq!(
        status,
        ArmStatus::Ran,
        "zero hits is a real result, never Unavailable"
    );
}

// ── Locale → drop list ───────────────────────────────────────────────────────

#[test]
fn stopwords_route_by_primary_subtag_and_never_fall_back_to_english() {
    use crate::commands::help::stopwords::{
        stopwords_for_locale, HELP_STOPWORDS_DE, HELP_STOPWORDS_EN,
    };

    assert_eq!(stopwords_for_locale("en"), HELP_STOPWORDS_EN);
    assert_eq!(stopwords_for_locale("de"), HELP_STOPWORDS_DE);
    // The renderer sends `i18n.language`, which carries a region on some
    // installs, and a hand-built agent-CLI body can send any casing.
    assert_eq!(stopwords_for_locale("de-AT"), HELP_STOPWORDS_DE);
    assert_eq!(stopwords_for_locale("EN"), HELP_STOPWORDS_EN);
    assert_eq!(stopwords_for_locale("en-GB"), HELP_STOPWORDS_EN);
    // A locale with no hand-curated list drops NOTHING. Empty, never
    // English: `in`/`an`/`es` are content words in other languages, so an
    // English fallback would silently delete real terms from a French or
    // Spanish question.
    assert!(
        stopwords_for_locale("fr").is_empty(),
        "an unknown locale must drop nothing — never the English list"
    );
    assert!(stopwords_for_locale("ja").is_empty());
}

#[test]
fn a_malformed_locale_is_an_unknown_one_not_an_error() {
    use crate::commands::help::stopwords::stopwords_for_locale;

    // Caller input (the agent CLI never sees the Zod cap), and none of these
    // may panic, allocate a copy of themselves, or resolve to a real list.
    for locale in [
        "",
        "-",
        "e n",
        "en_US",                 // `_` is not a BCP-47 separator
        "de; DROP TABLE",        //
        "englishenglishenglish", // over the cap
        "德文",
    ] {
        assert!(
            stopwords_for_locale(locale).is_empty(),
            "`{locale}` must resolve to no drop list rather than a refusal or a wrong one"
        );
    }
    // …and a huge one is rejected on length before anything is allocated
    // from it.
    assert!(stopwords_for_locale(&"e".repeat(100_000)).is_empty());
}

/// The all-dropped fallback, through the REAL arm rather than through
/// `retrieval::lexical` directly: a question made only of function words must
/// still return hits. Without the fallback it sanitizes to the empty string,
/// `search_any` answers zero hits, and the arm reports `Ran` — a silent empty
/// result on the ONE arm a default install runs.
#[test]
fn a_question_made_only_of_function_words_still_returns_hits() {
    let (ranks, status) = run_lexical_arm(&corpus(), "What is it?", 3, Some("en"));
    assert_eq!(status, ArmStatus::Ran);
    assert!(
        !ranks.is_empty(),
        "an all-stopword question must fall back to its unfiltered tokens, not answer nothing"
    );
}

/// Two entries sharing function words ("What", "do", "is") — only `asked` is
/// about exporting, so only it should match once the drop list is applied.
fn drop_list_entries() -> Vec<HelpSearchRequestEntry> {
    vec![
        entry(
            "unrelated",
            "What is Autopilot?",
            "It watches a saved search and scores new postings for you.",
        ),
        entry(
            "asked",
            "How do I export a finished document?",
            "Press Export above a finished document and choose PDF, DOCX or TXT.",
        ),
    ]
}

/// The drop list is only worth having if it changes what the arm returns.
/// Anchored on an ABSOLUTE, not on a comparison of two derived numbers: the
/// unrelated entry must be present without the list and absent with it.
#[test]
fn the_english_drop_list_keeps_a_questions_function_words_from_pulling_in_an_entry() {
    let entries = drop_list_entries();
    let query = "What do I do to export it?";

    // "xx" is a well-formed tag with no list — the no-filtering baseline.
    let (unfiltered, _) = run_lexical_arm(&entries, query, 3, Some("xx"));
    assert!(
        unfiltered.contains(&"unrelated".to_string()),
        "premise: unfiltered, `What`/`do`/`is` alone match the unrelated entry; got {unfiltered:?}"
    );

    let (filtered, status) = run_lexical_arm(&entries, query, 3, Some("en"));
    assert_eq!(status, ArmStatus::Ran);
    assert_eq!(
        filtered,
        vec!["asked".to_string()],
        "only `export` survives the drop list, so only the entry about exporting matches"
    );
}

/// An OMITTED `locale` must drop NOTHING, never fall back to English.
///
/// Both places that could quietly supply an English default are on this path
/// and both are exercised: serde (the generated contract carried a
/// `#[serde(default)] = "en"` until `HelpSearchRequestSchema.locale` became
/// optional — so the request is built by DESERIALIZING a body with no
/// `locale` key, not by naming the field) and `run_lexical_arm`'s own
/// unwrapping of the `Option`.
///
/// Anchored on an ABSOLUTE, the same way the test above is: the unrelated
/// entry must be PRESENT for an omitted locale (nothing dropped) and ABSENT
/// for `en`. Comparing the two result lists to each other would pass for any
/// pair of defaults that happened to agree.
///
/// Mutation-visible: `locale.unwrap_or_default()` → `unwrap_or("en")` in
/// `run_lexical_arm` and the first assertion fails.
#[test]
fn an_omitted_locale_drops_nothing_rather_than_defaulting_to_english() {
    let req: HelpSearchRequest = serde_json::from_value(serde_json::json!({
        "query": "What do I do to export it?",
        "entries": [{ "id": "unrelated", "title": "t", "body": "b" }],
    }))
    .expect("`locale` is optional on the wire");
    assert!(
        req.locale.is_none(),
        "premise: an absent `locale` key must deserialize to None — no serde default may \
         invent one"
    );

    let entries = drop_list_entries();
    let query = "What do I do to export it?";

    let (omitted, status) = run_lexical_arm(&entries, query, 3, req.locale.as_deref());
    assert_eq!(status, ArmStatus::Ran);
    assert!(
        omitted.contains(&"unrelated".to_string()),
        "a caller that never said which language its entries are in has not said English: \
         its function words must still match, got {omitted:?}"
    );
    let (english, _) = run_lexical_arm(&entries, query, 3, Some("en"));
    assert!(
        !english.contains(&"unrelated".to_string()),
        "premise: declaring `en` IS what drops them; got {english:?}"
    );
}
