use std::sync::atomic::{AtomicUsize, Ordering};

use tempfile::TempDir;

use super::*;
use crate::commands::ai_provider::{EmbeddingSpace, EMBEDDING_VECTOR_VERSION};

// ── Fixtures ─────────────────────────────────────────────────────────────────

fn entry(id: &str, title: &str, body: &str) -> HelpSearchRequestEntry {
    HelpSearchRequestEntry {
        id: id.to_string(),
        title: title.to_string(),
        body: body.to_string(),
    }
}

/// A small, realistic corpus: three entries whose ANSWERS carry wording the
/// questions do not, so a query written against an answer can only reach its
/// entry through the `description` column.
fn corpus() -> Vec<HelpSearchRequestEntry> {
    vec![
        entry(
            "documentsQuestions.importFormats",
            "Which file formats can I import?",
            "Drop a file onto Resume Management: the hint reads PDF, DOC, DOCX, and TXT files \
             are accepted too. A scanned page has no text layer to extract.",
        ),
        entry(
            "aiGenerateQuestions.exportDoc",
            "How do I export a finished document?",
            "Press Export above a finished document and choose PDF, DOCX or TXT.",
        ),
        entry(
            "privacyQuestions.whatLeaves",
            "What data leaves my computer?",
            "Your documents and applications stay in local files on your machine.",
        ),
    ]
}

fn cfg(provider: &str, model: &str) -> EmbeddingConfig {
    EmbeddingConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        base_url: None,
    }
}

fn vector(cfg: &EmbeddingConfig, values: Vec<f64>) -> EmbeddingVector {
    let dim = values.len();
    EmbeddingVector {
        values,
        space: EmbeddingSpace {
            provider: cfg.provider.clone(),
            model: cfg.model.clone(),
            dim,
            version: EMBEDDING_VECTOR_VERSION,
        },
    }
}

fn store() -> (TempDir, DocumentStore) {
    let dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&dir.path().to_path_buf()).unwrap();
    (dir, store)
}

/// A scripted [`Embedder`] that counts its round-trips — the seam that makes
/// "how many provider calls did this search make" a test rather than a claim.
///
/// `values` is keyed by call ORDER, not by text: the first call is always the
/// query, so a fixed vector per call index is enough to control the cosine
/// ordering deterministically without re-implementing an embedding model.
struct ScriptedEmbedder {
    calls: AtomicUsize,
    /// One vector per call, in order. A call past the end returns the last.
    values: Vec<Vec<f64>>,
    cfg: EmbeddingConfig,
    /// When true every call fails, standing in for an unreachable provider.
    fails: bool,
}

impl ScriptedEmbedder {
    fn new(cfg: &EmbeddingConfig, values: Vec<Vec<f64>>) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            values,
            cfg: cfg.clone(),
            fails: false,
        }
    }

    fn failing(cfg: &EmbeddingConfig) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            values: Vec::new(),
            cfg: cfg.clone(),
            fails: true,
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Embedder for ScriptedEmbedder {
    async fn embed_one(&self, _text: &str) -> Option<EmbeddingVector> {
        let i = self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fails {
            return None;
        }
        let values = self
            .values
            .get(i)
            .or_else(|| self.values.last())
            .cloned()
            .unwrap_or_else(|| vec![1.0, 0.0]);
        Some(vector(&self.cfg, values))
    }
}

/// `enable_time` because the in-flight-cancel test below needs a timer for
/// both halves of what it asserts: the `tokio::time::timeout` that turns
/// "hangs forever" into a failed assertion, and the sleep that lands the
/// cancel AFTER the embed is already in flight. Harmless for every other test
/// here — none of them arm a timer.
fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(f)
}

/// [`run_dense_arm`] on PRODUCTION's own wall-clock budget — the two tests
/// that measure the budget call `run_dense_arm` directly with a short one
/// instead (see [`run_dense_arm`]'s own doc for why it is a parameter).
fn arm<E: Embedder + ?Sized>(
    store: &DocumentStore,
    active: &EmbeddingConfig,
    embedder: &E,
    query: &str,
    entries: &[HelpSearchRequestEntry],
) -> (Vec<String>, ArmStatus) {
    block_on(run_dense_arm(
        store,
        active,
        embedder,
        query,
        entries,
        crate::commands::ai_provider::timeouts::DENSE_ARM_TIMEOUT,
        // An unregistered token nobody can fire — what a caller that sends no
        // `queryId` gets. The cancellation tests below pass their own.
        &CancellationToken::new(),
    ))
}

/// An [`Embedder`] that CANCELS its own token after `cancel_after` calls —
/// the only way to observe "a cancel arriving mid-arm" deterministically,
/// since a real one arrives from another task at an unpredictable moment.
/// Counts calls like [`ScriptedEmbedder`] does.
struct CancellingEmbedder {
    calls: AtomicUsize,
    cfg: EmbeddingConfig,
    token: CancellationToken,
    cancel_after: usize,
}

#[async_trait]
impl Embedder for CancellingEmbedder {
    async fn embed_one(&self, _text: &str) -> Option<EmbeddingVector> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if n >= self.cancel_after {
            self.token.cancel();
        }
        Some(vector(&self.cfg, vec![1.0, 0.0]))
    }
}

/// An [`Embedder`] whose every round-trip takes `delay` — a slow provider,
/// without a slow test: the wall-clock budget is injected, not waited out.
struct SlowEmbedder {
    calls: AtomicUsize,
    cfg: EmbeddingConfig,
    delay: std::time::Duration,
}

#[async_trait]
impl Embedder for SlowEmbedder {
    async fn embed_one(&self, _text: &str) -> Option<EmbeddingVector> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        // `std::thread::sleep`, not `tokio::time::sleep`: the bound this fake
        // exercises is `std::time::Instant::elapsed`, which only real time
        // advances, and blocking IS what a slow embed does to this arm anyway.
        std::thread::sleep(self.delay);
        Some(vector(&self.cfg, vec![1.0, 0.0]))
    }
}

// ── Boundary validation ──────────────────────────────────────────────────────

#[test]
fn an_empty_query_is_refused() {
    // The command trims first, so a whitespace-only query arrives here empty.
    let err = validate("", &corpus()).unwrap_err();
    assert!(
        matches!(err, AppError::Validation(_)),
        "expected a validation error, got {err:?}"
    );
}

#[test]
fn a_query_past_the_cap_is_refused() {
    let long = "x".repeat(QUERY_MAX_CHARS + 1);
    assert!(validate(&long, &corpus()).is_err());
    // The cap itself is inclusive — exactly at the limit must still pass.
    assert!(validate(&"x".repeat(QUERY_MAX_CHARS), &corpus()).is_ok());
}

#[test]
fn the_query_cap_counts_chars_not_bytes() {
    // A multi-byte query at the char cap must not be refused for its byte
    // length: the Zod cap the renderer enforces is a CHAR cap, so a
    // byte-counting check here would refuse a legitimate German or Japanese
    // question that the schema accepted.
    let multibyte = "ü".repeat(QUERY_MAX_CHARS);
    assert!(
        multibyte.len() > QUERY_MAX_CHARS,
        "fixture must be multi-byte"
    );
    assert!(validate(&multibyte, &corpus()).is_ok());
}

#[test]
fn an_empty_entry_list_is_refused() {
    assert!(validate("how do i export", &[]).is_err());
}

#[test]
fn too_many_entries_are_refused() {
    let many: Vec<HelpSearchRequestEntry> = (0..=ENTRIES_MAX)
        .map(|i| entry(&format!("s.e{i}"), "title", "body"))
        .collect();
    assert!(many.len() > ENTRIES_MAX);
    assert!(validate("export", &many).is_err());
    assert!(validate("export", &many[..ENTRIES_MAX]).is_ok());
}

#[test]
fn an_oversized_entry_body_is_refused() {
    let entries = vec![entry("s.e", "title", &"x".repeat(ENTRY_BODY_MAX_CHARS + 1))];
    assert!(validate("export", &entries).is_err());
}

#[test]
fn an_oversized_entry_title_is_refused() {
    let entries = vec![entry("s.e", &"x".repeat(ENTRY_TITLE_MAX_CHARS + 1), "body")];
    assert!(validate("export", &entries).is_err());
}

#[test]
fn an_oversized_or_empty_entry_id_is_refused() {
    assert!(validate("export", &[entry("", "title", "body")]).is_err());
    assert!(validate(
        "export",
        &[entry(&"a".repeat(ENTRY_ID_MAX_CHARS + 1), "title", "body")]
    )
    .is_err());
}

#[test]
fn an_entry_id_outside_the_schemas_charset_is_refused() {
    // Ids are echoed straight back to the caller. Anything that could not
    // have come from a translation leaf path is refused rather than
    // round-tripped — the schema's own `^[A-Za-z0-9_.-]+$`.
    for bad in ["a b", "a/b", "a<b", "señor"] {
        assert!(
            validate("export", &[entry(bad, "title", "body")]).is_err(),
            "id `{bad}` should be refused"
        );
    }
    assert!(validate(
        "export",
        &[entry("aiGenerateQuestions.export-doc_1", "t", "b")]
    )
    .is_ok());
}

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
    use super::stopwords::{stopwords_for_locale, HELP_STOPWORDS_DE, HELP_STOPWORDS_EN};

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
    use super::stopwords::stopwords_for_locale;

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

/// The drop list is only worth having if it changes what the arm returns.
/// Anchored on an ABSOLUTE, not on a comparison of two derived numbers: the
/// unrelated entry must be present without the list and absent with it.
#[test]
fn the_english_drop_list_keeps_a_questions_function_words_from_pulling_in_an_entry() {
    let entries = vec![
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
    ];
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

    let entries = vec![
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
    ];
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

// ── Fusion + reply assembly ──────────────────────────────────────────────────

fn ids(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_string()).collect()
}

#[test]
fn the_reply_is_truncated_to_the_requested_limit() {
    let result = assemble(
        (ids(&["a", "b", "c", "d"]), ArmStatus::Ran),
        (Vec::new(), ArmStatus::Skipped),
        2,
    );
    assert_eq!(result.results.len(), 2);
    assert_eq!(result.results[0].id, "a");
    assert_eq!(result.results[1].id, "b");
}

#[test]
fn an_entry_both_arms_found_outranks_one_only_the_lexical_arm_found() {
    // The whole point of fusing: `b` is only SECOND in the lexical list, but
    // it is the one entry both arms surfaced, so it must come out on top of
    // `a` (lexical rank 1, absent from the dense list). An implementation
    // that concatenated the arms, or that let one arm win outright, would
    // return `a` first.
    let result = assemble(
        (ids(&["a", "b"]), ArmStatus::Ran),
        (ids(&["b", "c"]), ArmStatus::Ran),
        3,
    );
    let order: Vec<&str> = result.results.iter().map(|h| h.id.as_str()).collect();
    assert_eq!(order, vec!["b", "a", "c"], "scores: {:?}", result.results);
    let scores: Vec<f64> = result.results.iter().map(|h| h.score).collect();
    assert!(
        scores.windows(2).all(|w| w[0] >= w[1]),
        "results must be best-first: {scores:?}"
    );
    // The score is RRF's, not a rank index or a BM25/cosine value: an id in
    // both lists scores 1/(60+1) + 1/(60+2), an id in one scores 1/(60+1).
    let expected_b = 1.0 / (fusion::RRF_K + 2.0) + 1.0 / (fusion::RRF_K + 1.0);
    assert!(
        (result.results[0].score - expected_b).abs() < 1e-12,
        "expected the RRF score {expected_b}, got {}",
        result.results[0].score
    );
}

#[test]
fn keyword_results_still_come_back_when_the_dense_arm_is_unavailable() {
    let result = assemble(
        (ids(&["a", "b"]), ArmStatus::Ran),
        (Vec::new(), ArmStatus::Unavailable),
        3,
    );
    assert_eq!(
        result.results.len(),
        2,
        "an embedding failure must not empty the reply"
    );
    assert_eq!(result.mode, HelpSearchMode::Keyword);
    assert_eq!(result.arms.dense, ArmStatus::Unavailable);
    assert_eq!(result.arms.lexical, ArmStatus::Ran);
}

#[test]
fn mode_is_hybrid_only_when_the_dense_arm_actually_ran() {
    assert_eq!(mode_of(ArmStatus::Ran), HelpSearchMode::Hybrid);
    assert_eq!(
        mode_of(ArmStatus::Skipped),
        HelpSearchMode::Keyword,
        "the preference being off is keyword results, not hybrid ones"
    );
    assert_eq!(
        mode_of(ArmStatus::Unavailable),
        HelpSearchMode::Keyword,
        "an embedding failure is keyword results, not hybrid ones"
    );
}

#[test]
fn help_arm_statuses_serialize_as_the_wire_contract_tags() {
    // `ArmStatus` is shared with `hybrid_search`, so pin the exact tags
    // `HelpSearchResultSchema`'s `z.enum`s declare — a variant added or
    // renamed over there would otherwise widen this command's wire contract
    // silently.
    let json = serde_json::to_value(assemble(
        (ids(&["a"]), ArmStatus::Ran),
        (Vec::new(), ArmStatus::Skipped),
        1,
    ))
    .unwrap();
    assert_eq!(json["mode"], "keyword");
    assert_eq!(json["arms"]["lexical"], "ran");
    assert_eq!(json["arms"]["dense"], "skipped");
    assert_eq!(json["results"][0]["id"], "a");
    assert!(json["results"][0]["score"].is_number());

    let hybrid = serde_json::to_value(assemble(
        (ids(&["a"]), ArmStatus::Ran),
        (ids(&["a"]), ArmStatus::Ran),
        1,
    ))
    .unwrap();
    assert_eq!(hybrid["mode"], "hybrid");
    let unavailable = serde_json::to_value(assemble(
        (Vec::new(), ArmStatus::Unavailable),
        (Vec::new(), ArmStatus::Unavailable),
        1,
    ))
    .unwrap();
    assert_eq!(unavailable["arms"]["lexical"], "unavailable");
    assert_eq!(unavailable["arms"]["dense"], "unavailable");
}

// ── Dense arm ────────────────────────────────────────────────────────────────

#[test]
fn a_failed_query_embed_is_unavailable_and_embeds_no_entries() {
    let (_dir, store) = store();
    let active = cfg("ollama", "nomic-embed-text");
    let embedder = ScriptedEmbedder::failing(&active);

    let (ranks, status) = arm(&store, &active, &embedder, "how do i export", &corpus());

    assert!(ranks.is_empty());
    assert_eq!(status, ArmStatus::Unavailable);
    assert_eq!(
        embedder.calls(),
        1,
        "a failed QUERY embed must abandon the arm, never go on to embed 3 entries"
    );
}

#[test]
fn a_cold_cache_embeds_the_query_and_every_entry_once_then_persists_them() {
    let (_dir, store) = store();
    let active = cfg("ollama", "nomic-embed-text");
    let embedder = ScriptedEmbedder::new(&active, vec![vec![1.0, 0.0]]);
    let entries = corpus();

    let (ranks, status) = arm(&store, &active, &embedder, "q", &entries);

    assert_eq!(status, ArmStatus::Ran);
    assert_eq!(ranks.len(), entries.len());
    assert_eq!(
        embedder.calls(),
        1 + entries.len(),
        "one query embed plus one per entry"
    );
    for e in &entries {
        assert!(
            store
                .get_help_vector(&sha256_hex(&e.body), &active)
                .is_some(),
            "{} must be cached after the run",
            e.id
        );
    }
}

#[test]
fn a_warm_cache_embeds_only_the_query() {
    let (_dir, store) = store();
    let active = cfg("ollama", "nomic-embed-text");
    let entries = corpus();
    // Pre-seed every entry, exactly as a previous run would have.
    for e in &entries {
        store
            .upsert_help_vector(&sha256_hex(&e.body), &vector(&active, vec![0.0, 1.0]))
            .unwrap();
    }
    let embedder = ScriptedEmbedder::new(&active, vec![vec![1.0, 0.0]]);

    let (ranks, status) = arm(&store, &active, &embedder, "q", &entries);

    assert_eq!(status, ArmStatus::Ran);
    assert_eq!(ranks.len(), entries.len());
    assert_eq!(
        embedder.calls(),
        1,
        "the query is embedded per request; cached entries must cost nothing"
    );
}

#[test]
fn an_edited_answer_is_a_cache_miss_and_re_embeds_only_that_entry() {
    let (_dir, store) = store();
    let active = cfg("ollama", "nomic-embed-text");
    let mut entries = corpus();
    for e in &entries {
        store
            .upsert_help_vector(&sha256_hex(&e.body), &vector(&active, vec![0.0, 1.0]))
            .unwrap();
    }
    // The cache keys on the BODY hash, so editing one answer must invalidate
    // exactly that row — no id, locale or version bump involved.
    entries[1].body = "Press Export and choose PDF, DOCX, TXT or Markdown.".to_string();
    let embedder = ScriptedEmbedder::new(&active, vec![vec![1.0, 0.0]]);

    let (_, status) = arm(&store, &active, &embedder, "q", &entries);

    assert_eq!(status, ArmStatus::Ran);
    assert_eq!(
        embedder.calls(),
        2,
        "the query plus the ONE edited entry — an unchanged answer must stay a hit"
    );
}

#[test]
fn a_row_from_another_embedding_space_is_a_miss_and_is_re_embedded() {
    let (_dir, store) = store();
    let old_space = cfg("openai", "text-embedding-3-small");
    let active = cfg("ollama", "nomic-embed-text");
    let entries = corpus();
    for e in &entries {
        store
            .upsert_help_vector(&sha256_hex(&e.body), &vector(&old_space, vec![0.0, 1.0]))
            .unwrap();
    }
    let embedder = ScriptedEmbedder::new(&active, vec![vec![1.0, 0.0]]);

    let (ranks, status) = arm(&store, &active, &embedder, "q", &entries);

    assert_eq!(status, ArmStatus::Ran);
    assert_eq!(
        embedder.calls(),
        1 + entries.len(),
        "every row was written in a DIFFERENT embedding space, so none may be reused"
    );
    assert_eq!(ranks.len(), entries.len());
    // And the miss re-wrote each row into the active space.
    for e in &entries {
        let v = store
            .get_help_vector(&sha256_hex(&e.body), &active)
            .expect("re-embedded row is now readable in the active space");
        assert_eq!(v.space.provider, "ollama");
    }
}

#[test]
fn entry_embeds_that_all_fail_leave_the_arm_unavailable_not_falsely_ran() {
    // The query embeds fine, every entry embed fails: there is nothing to
    // rank, so the arm must say `unavailable` rather than `ran` with an empty
    // list (which would make the reply claim `mode: hybrid`).
    struct QueryOnly(AtomicUsize, EmbeddingConfig);
    #[async_trait]
    impl Embedder for QueryOnly {
        async fn embed_one(&self, _text: &str) -> Option<EmbeddingVector> {
            if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                Some(vector(&self.1, vec![1.0, 0.0]))
            } else {
                None
            }
        }
    }
    let (_dir, store) = store();
    let active = cfg("ollama", "nomic-embed-text");
    let embedder = QueryOnly(AtomicUsize::new(0), active.clone());

    let (ranks, status) = arm(&store, &active, &embedder, "q", &corpus());

    assert!(ranks.is_empty());
    assert_eq!(status, ArmStatus::Unavailable);
}

/// A PARTIAL pairing is `unavailable`, never a `hybrid` reply ranked by half
/// a corpus: one entry's embed fails, the other two succeed, and the arm must
/// still refuse rather than hand back a two-entry dense ranking the reply
/// would then label `mode: hybrid`.
#[test]
fn one_failed_entry_embed_is_unavailable_not_a_partly_ranked_hybrid() {
    /// Query + entry 1 succeed, entry 2 fails, entry 3 succeeds.
    struct OneFails(AtomicUsize, EmbeddingConfig);
    #[async_trait]
    impl Embedder for OneFails {
        async fn embed_one(&self, _text: &str) -> Option<EmbeddingVector> {
            let i = self.0.fetch_add(1, Ordering::SeqCst);
            if i == 2 {
                return None;
            }
            Some(vector(&self.1, vec![1.0, 0.0]))
        }
    }
    let (_dir, store) = store();
    let active = cfg("ollama", "nomic-embed-text");
    let embedder = OneFails(AtomicUsize::new(0), active.clone());
    let entries = corpus();

    let (ranks, status) = arm(&store, &active, &embedder, "q", &entries);

    assert_eq!(
        status,
        ArmStatus::Unavailable,
        "2 of 3 entries paired — the arm must not report `ran`"
    );
    assert!(
        ranks.is_empty(),
        "and it must not hand back the partial ranking either, or the fused order would be \
         part-semantic under a `keyword` label: {ranks:?}"
    );
    // The two successful embeds are still cached, so the next question is warm
    // rather than the work being thrown away.
    assert!(store
        .get_help_vector(&sha256_hex(&entries[0].body), &active)
        .is_some());
}

/// The same rule one step further in: a vector that arrives but cannot be
/// SCORED. `dense_pair` compares embedding SPACES, so an all-zero vector of the
/// right dimension pairs perfectly well and is then dropped by `dense::cosine`
/// (zero magnitude — no direction to compare against). Counting PAIRS rather
/// than RANKS let that entry satisfy the all-or-nothing check and then vanish
/// from the ranking, which is precisely the partial ranking labelled `hybrid`
/// the rule exists to prevent.
///
/// Mutation-visible: restore `if pairs.len() < entries.len()` and this comes
/// back `Ran` with two of the three entries ranked.
#[test]
fn a_zero_vector_for_one_entry_is_unavailable_not_a_partly_ranked_hybrid() {
    /// Query + entries 1 and 3 embed normally; entry 2 comes back all zeros.
    struct ZeroForOne(AtomicUsize, EmbeddingConfig);
    #[async_trait]
    impl Embedder for ZeroForOne {
        async fn embed_one(&self, _text: &str) -> Option<EmbeddingVector> {
            let i = self.0.fetch_add(1, Ordering::SeqCst);
            // Call 0 is the query, so call 2 is the SECOND entry.
            let values = if i == 2 {
                vec![0.0, 0.0]
            } else {
                vec![1.0, 0.0]
            };
            Some(vector(&self.1, values))
        }
    }
    let (_dir, store) = store();
    let active = cfg("ollama", "nomic-embed-text");
    let embedder = ZeroForOne(AtomicUsize::new(0), active.clone());
    let entries = corpus();

    let (ranks, status) = arm(&store, &active, &embedder, "q", &entries);

    assert_eq!(
        status,
        ArmStatus::Unavailable,
        "an entry whose vector cannot be scored leaves the corpus part-ranked — the arm must \
         not report `ran`"
    );
    assert!(ranks.is_empty(), "and hand back no ranks either: {ranks:?}");
    // The distinction under test: this embed SUCCEEDED (it is cached, unlike a
    // failed one), so nothing before the ranking step could have caught it.
    assert!(
        store
            .get_help_vector(&sha256_hex(&entries[1].body), &active)
            .is_some(),
        "the zero vector must have been a successful, cached embed — otherwise this measures \
         the failed-embed path instead"
    );
}

/// The wall-clock bound: with a budget shorter than two embeds, the loop must
/// stop early rather than run `entries.len()` × the per-embed timeout — the
/// only thing that stops it at all, since v1 has no cancellation token.
/// Mutation-visible: drop the `break` and the embedder is called once per
/// entry and the arm reports `ran`.
#[test]
fn the_wall_clock_budget_stops_the_entry_loop_and_the_arm_reports_unavailable() {
    let (_dir, store) = store();
    let active = cfg("ollama", "nomic-embed-text");
    let embedder = SlowEmbedder {
        calls: AtomicUsize::new(0),
        cfg: active.clone(),
        delay: std::time::Duration::from_millis(40),
    };
    let entries = corpus();

    let (ranks, status) = block_on(run_dense_arm(
        &store,
        &active,
        &embedder,
        "q",
        &entries,
        // Spent by the query embed plus the first entry's.
        std::time::Duration::from_millis(50),
        &CancellationToken::new(),
    ));

    assert_eq!(status, ArmStatus::Unavailable);
    assert!(ranks.is_empty());
    assert!(
        embedder.calls.load(Ordering::SeqCst) < 1 + entries.len(),
        "the loop must have stopped before embedding every entry; it made {} calls for {} \
         entries",
        embedder.calls.load(Ordering::SeqCst),
        entries.len()
    );
}

/// The per-request miss budget: a caller may send up to `ENTRIES_MAX` entries,
/// so without this one call could charge `ENTRIES_MAX` embeds and write that
/// many permanent cache rows — repeatably. Past the cap the remaining entries
/// are lexical-only and the arm says so.
#[test]
fn the_cache_miss_budget_caps_the_embeds_one_request_can_make() {
    let (_dir, store) = store();
    let active = cfg("ollama", "nomic-embed-text");
    let over: Vec<HelpSearchRequestEntry> = (0..HELP_EMBED_MISSES_MAX + 5)
        .map(|i| entry(&format!("s.e{i}"), "title", &format!("body number {i}")))
        .collect();
    let embedder = ScriptedEmbedder::new(&active, vec![vec![1.0, 0.0]]);

    let (ranks, status) = arm(&store, &active, &embedder, "q", &over);

    assert_eq!(
        embedder.calls(),
        1 + HELP_EMBED_MISSES_MAX,
        "the query plus exactly the budget — never one embed per requested entry"
    );
    assert_eq!(
        status,
        ArmStatus::Unavailable,
        "entries left unembedded means the arm did not rank the corpus it was given"
    );
    assert!(ranks.is_empty());
}

/// The other side of the same bound: a request AT the budget is a normal,
/// fully-ranked run, so the cap can never be the reason a real question
/// (~51 shipped entries) degrades.
#[test]
fn a_request_at_the_miss_budget_still_runs_the_arm() {
    let (_dir, store) = store();
    let active = cfg("ollama", "nomic-embed-text");
    let exact: Vec<HelpSearchRequestEntry> = (0..HELP_EMBED_MISSES_MAX)
        .map(|i| entry(&format!("s.e{i}"), "title", &format!("body number {i}")))
        .collect();
    let embedder = ScriptedEmbedder::new(&active, vec![vec![1.0, 0.0]]);

    let (ranks, status) = arm(&store, &active, &embedder, "q", &exact);

    assert_eq!(status, ArmStatus::Ran);
    assert_eq!(ranks.len(), exact.len());
    assert_eq!(embedder.calls(), 1 + HELP_EMBED_MISSES_MAX);
}

/// The budget counts MISSES, not entries: a request larger than the cap whose
/// entries are already cached costs nothing and still ranks in full.
#[test]
fn cached_entries_do_not_consume_the_miss_budget() {
    let (_dir, store) = store();
    let active = cfg("ollama", "nomic-embed-text");
    let many: Vec<HelpSearchRequestEntry> = (0..HELP_EMBED_MISSES_MAX + 5)
        .map(|i| entry(&format!("s.e{i}"), "title", &format!("body number {i}")))
        .collect();
    for e in &many {
        store
            .upsert_help_vector(&sha256_hex(&e.body), &vector(&active, vec![0.0, 1.0]))
            .unwrap();
    }
    let embedder = ScriptedEmbedder::new(&active, vec![vec![1.0, 0.0]]);

    let (ranks, status) = arm(&store, &active, &embedder, "q", &many);

    assert_eq!(status, ArmStatus::Ran);
    assert_eq!(ranks.len(), many.len());
    assert_eq!(
        embedder.calls(),
        1,
        "only the query is embedded per request"
    );
}

#[test]
fn a_vector_from_another_space_is_never_scored_against_the_query() {
    // Belt-and-braces on top of `get_help_vector`'s own space check: a FRESH
    // embed that comes back tagged with a different space (a provider swapped
    // underneath us) must be dropped by `dense_pair`, not ranked.
    let (_dir, store) = store();
    let active = cfg("ollama", "nomic-embed-text");
    // The embedder answers in a DIFFERENT space than `active`.
    let embedder = ScriptedEmbedder::new(&cfg("openai", "text-embedding-3-small"), vec![vec![1.0]]);

    let (ranks, status) = arm(&store, &active, &embedder, "q", &corpus());

    // The query and the entries all come back in the same (wrong) space here,
    // so they DO pair with each other — the real cross-space case is the
    // cached one above. What must never happen is a panic or a silent
    // dimension-mismatch score.
    assert_eq!(status, ArmStatus::Ran);
    assert_eq!(ranks.len(), 3);

    // Now the genuinely mixed case: a cached row in the active space, a query
    // vector from another one.
    let mismatched = vector(&cfg("openai", "text-embedding-3-small"), vec![1.0, 0.0]);
    assert!(
        dense_pair("id", &vector(&active, vec![1.0, 0.0]).space, &mismatched).is_none(),
        "two vectors from different embedding spaces must never be scored together"
    );
}

// ── The production gate for the dense arm ────────────────────────────────────

/// The same deletion-guard shape as the eviction test below, for the same
/// reason: `help_search` needs a running Tauri app, so the ONE `if
/// semantic_on(&app)` that decides whether a question ever reaches a paid
/// embedding provider cannot be exercised from a unit test. What CAN be checked
/// is that the dense arm's only call site still sits inside that gate.
///
/// Without this, un-gating `run_dense` — the whole "semantic OFF makes zero
/// embed calls" property, and the default-install posture behind it — would
/// leave nothing red anywhere: every dense-arm test calls `run_dense_arm`
/// directly, below the gate.
#[test]
fn the_dense_arm_call_still_sits_inside_the_semantic_gate() {
    const HELP_SRC: &str =
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/commands/help.rs"));
    let start = HELP_SRC
        .find("let dense = if semantic_on(&app) {")
        .expect("`let dense = if semantic_on(&app) {` still guards the dense arm in help_search");
    let rest = &HELP_SRC[start..];
    let end = rest
        .find("} else {")
        .expect("the gate has an else branch (the `skipped` arm)");
    let gated = &rest[..end];
    assert!(
        gated.contains("run_dense(&app,"),
        "the dense arm must be called INSIDE the semantic_on gate; branch body was:\n{gated}"
    );
    assert_eq!(
        HELP_SRC.matches("run_dense(&app").count(),
        1,
        "and that must remain its ONLY call site — a second, ungated one would spend against \
         the provider with the preference off"
    );
}

// ── The eviction call site ───────────────────────────────────────────────────

/// A deletion guard, not a semantics proof: `ai_set_embedding_config` needs a
/// running Tauri app, so its body cannot be called from a unit test. What CAN
/// be checked is that the line still sits inside the `space_changed` branch
/// next to its two siblings — the same `include_str!` shape
/// `agent_cli::policy`'s exactness test uses for `lib.rs`.
///
/// Without this, dropping `clear_help_vectors()` from that branch would leave
/// every help vector stranded in a space nothing can read, and no test
/// anywhere would go red.
#[test]
fn the_embedding_space_change_branch_still_clears_the_help_vector_cache() {
    const AI_MOD: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/commands/ai/mod.rs"
    ));
    let start = AI_MOD
        .find("if space_changed {")
        .expect("`if space_changed {` still exists in ai_set_embedding_config");
    let rest = &AI_MOD[start..];
    // The branch's body ends at the success payload that follows it.
    let end = rest
        .find("json!(")
        .expect("the branch is followed by its json! reply");
    let branch = &rest[..end];
    for needle in [
        "clear_posting_vectors",
        "clear_match_scores",
        "clear_help_vectors",
    ] {
        assert!(
            branch.contains(needle),
            "`{needle}` must be evicted in the space-changed branch; branch body was:\n{branch}"
        );
    }
}

// ── Cancellation ─────────────────────────────────────────────────────────────

#[test]
fn a_help_prefixed_query_id_is_accepted_and_every_other_shape_is_refused() {
    // Optional: an agent-CLI caller that sends no id gets no cancellation,
    // not a refusal.
    assert!(validate_query_id(None).is_ok());
    assert!(validate_query_id(Some("help-0d3f")).is_ok());
    assert!(
        validate_query_id(Some(&format!("{QUERY_ID_PREFIX}{}", "x".repeat(59)))).is_ok(),
        "exactly at the cap must pass — the cap is inclusive"
    );

    for bad in [
        // The postings search's own prefix: a shared id space where either
        // feature could name the other's live search is exactly what the
        // prefixes exist to prevent.
        "search-0d3f",
        // A Rust-minted job id — the collision that would replace a live
        // scrape's token and then delete its slot.
        "job-0d3f",
        "run-0d3f",
        "",
        "0d3f",
        "HELP-0d3f",
    ] {
        assert!(
            matches!(validate_query_id(Some(bad)), Err(AppError::Validation(_))),
            "`{bad}` must be refused at the boundary"
        );
    }
    // 65 chars: one past the cap.
    let too_long = format!("{QUERY_ID_PREFIX}{}", "x".repeat(60));
    assert_eq!(too_long.chars().count(), QUERY_ID_MAX_CHARS + 1);
    assert!(validate_query_id(Some(&too_long)).is_err());
}

/// A cancel that landed before the arm started must cost ZERO provider calls
/// — not "one, then stop".
///
/// Mutation-visible: replace the query's `embed_or_cancel` with a bare
/// `embedder.embed_one(query).await` and the count goes to 1 while the STATUS
/// stays `Unavailable` (the loop's own `is_cancelled()` break still fires),
/// which is why this asserts the CALL COUNT and not the status alone. There
/// is deliberately no separate `is_cancelled()` guard above the query embed
/// to delete — the `biased` race IS that guard (see `run_dense_arm`).
#[test]
fn a_pre_cancelled_token_embeds_nothing_and_reports_unavailable() {
    let (_dir, store) = store();
    let active = cfg("ollama", "nomic-embed-text");
    let embedder = ScriptedEmbedder::new(&active, vec![vec![1.0, 0.0]]);
    let token = CancellationToken::new();
    token.cancel();

    let (ranks, status) = block_on(run_dense_arm(
        &store,
        &active,
        &embedder,
        "q",
        &corpus(),
        crate::commands::ai_provider::timeouts::DENSE_ARM_TIMEOUT,
        &token,
    ));

    assert_eq!(status, ArmStatus::Unavailable);
    assert!(ranks.is_empty());
    assert_eq!(
        embedder.calls(),
        0,
        "a cancel that arrived before the arm started must not reach the provider at all"
    );
}

/// A cancel arriving MID-arm stops the entry loop instead of running it out.
/// The fake cancels its own token on its 2nd call (the query embed is the
/// 1st), so the loop must stop after the entry that was already in flight —
/// never the full `1 + entries.len()`.
///
/// Mutation-visible: drop `|| token.is_cancelled()` from the loop guard and
/// the count runs to the end of the corpus.
#[test]
fn a_cancel_after_the_second_embed_stops_the_entry_loop() {
    let (_dir, store) = store();
    let active = cfg("ollama", "nomic-embed-text");
    let token = CancellationToken::new();
    let entries = corpus();
    let embedder = CancellingEmbedder {
        calls: AtomicUsize::new(0),
        cfg: active.clone(),
        token: token.clone(),
        cancel_after: 2,
    };

    let (ranks, status) = block_on(run_dense_arm(
        &store,
        &active,
        &embedder,
        "q",
        &entries,
        crate::commands::ai_provider::timeouts::DENSE_ARM_TIMEOUT,
        &token,
    ));

    assert_eq!(
        status,
        ArmStatus::Unavailable,
        "a cancelled arm is Unavailable — the caller still gets the keyword results"
    );
    assert!(ranks.is_empty(), "all-or-nothing: {ranks:?}");
    assert!(
        embedder.calls.load(Ordering::SeqCst) <= 3,
        "the loop must stop at the cancel, not run the corpus out: {} calls for {} entries",
        embedder.calls.load(Ordering::SeqCst),
        entries.len()
    );
    // …and the premise: without the cancel this corpus takes 1 + len calls.
    // A FRESH store — the run above already cached the entry it embedded, and
    // reusing that store would make the premise cheaper than it really is.
    let (_dir2, cold_store) = self::store();
    let uncancelled = ScriptedEmbedder::new(&active, vec![vec![1.0, 0.0]]);
    let (_, ok_status) = arm(&cold_store, &active, &uncancelled, "q", &entries);
    assert_eq!(
        ok_status,
        ArmStatus::Ran,
        "premise: the same corpus ranks fine when nothing cancels it"
    );
    assert_eq!(uncancelled.calls(), 1 + entries.len());
}

/// The case the between-entries token check exists for, and the ONE case the
/// per-embed race cannot cover: a cache HIT needs no embed, so it never
/// touches the token. Every entry but the first is pre-cached here and the
/// cancel lands during that one cold embed — which SUCCEEDS (the race polls
/// the token first, and it was still live at that moment). Without the loop's
/// `token.is_cancelled()` break the remaining hits pair from cache, the arm
/// ranks all three and reports `Ran`, putting `mode: "hybrid"` on the wire
/// for a search the user cancelled.
///
/// Mutation-visible: drop `|| token.is_cancelled()` from the loop guard and
/// this flips to `Ran` with three ranks.
#[test]
fn a_cancel_mid_arm_is_unavailable_even_when_every_remaining_entry_is_cached() {
    let (_dir, store) = store();
    let active = cfg("ollama", "nomic-embed-text");
    let entries = corpus();
    for e in &entries[1..] {
        store
            .upsert_help_vector(&sha256_hex(&e.body), &vector(&active, vec![1.0, 0.0]))
            .unwrap();
    }
    let token = CancellationToken::new();
    // Call 1 is the query, call 2 is the only COLD entry — cancel there.
    let embedder = CancellingEmbedder {
        calls: AtomicUsize::new(0),
        cfg: active.clone(),
        token: token.clone(),
        cancel_after: 2,
    };

    let (ranks, status) = block_on(run_dense_arm(
        &store,
        &active,
        &embedder,
        "q",
        &entries,
        crate::commands::ai_provider::timeouts::DENSE_ARM_TIMEOUT,
        &token,
    ));

    assert_eq!(
        embedder.calls.load(Ordering::SeqCst),
        2,
        "premise: only the query and the single cold entry are embedded — the rest are hits"
    );
    assert_eq!(
        status,
        ArmStatus::Unavailable,
        "a cancelled arm must never report Ran, even when the cache could complete it"
    );
    assert!(ranks.is_empty(), "all-or-nothing: {ranks:?}");
}

/// The half no other test here reaches: a cancel that arrives while an embed
/// is ALREADY in flight. Every other cancellation test lands the cancel
/// between calls, where a plain `.await` would look identical.
///
/// The fake never returns from `embed_one` (`std::future::pending` — a
/// oneshot nobody sends, without the channel), so with a bare
/// `embedder.embed_one(text).await` in `embed_or_cancel` this arm can only
/// end by running out the wall clock: `DENSE_ARM_TIMEOUT` is checked BETWEEN
/// entries and never interrupts a call, so nothing would ever cancel the
/// query embed at all. That is exactly the "sits waiting out the provider's
/// per-attempt timeout" behaviour the race exists to prevent, and the reason
/// the assertion is wrapped in `tokio::time::timeout`: the failure mode
/// under mutation is a HANG, and a hanging test is worse than no test.
///
/// The budget is 2 s against a cancel fired at 50 ms — two orders of
/// magnitude, so this measures the mechanism, not the scheduler.
///
/// Mutation-visible: replace the query's `embed_or_cancel` with
/// `embedder.embed_one(query).await` and this fails on the `expect` below
/// (the elapsed budget), rather than hanging the suite.
#[test]
fn a_cancel_of_an_in_flight_embed_returns_without_waiting_the_provider_out() {
    struct HangingEmbedder {
        calls: AtomicUsize,
    }
    #[async_trait]
    impl Embedder for HangingEmbedder {
        async fn embed_one(&self, _text: &str) -> Option<EmbeddingVector> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            // Never resolves — a provider call that has begun and will not
            // come back inside this test's lifetime.
            std::future::pending::<()>().await;
            None
        }
    }

    let (_dir, store) = store();
    let active = cfg("ollama", "nomic-embed-text");
    let embedder = HangingEmbedder {
        calls: AtomicUsize::new(0),
    };

    let outcome = block_on(async {
        let token = CancellationToken::new();
        let canceller = token.clone();
        // Fired from a separate task so the cancel lands while the arm is
        // parked inside the embed, not before it starts (which is what
        // `a_pre_cancelled_token_embeds_nothing_and_reports_unavailable`
        // already covers).
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            canceller.cancel();
        });
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            run_dense_arm(
                &store,
                &active,
                &embedder,
                "q",
                &corpus(),
                crate::commands::ai_provider::timeouts::DENSE_ARM_TIMEOUT,
                &token,
            ),
        )
        .await
    });

    let (ranks, status) = outcome.expect(
        "a cancel must abandon an IN-FLIGHT embed, not wait it out: the arm was still \
         running 2s after a cancel fired at 50ms",
    );
    assert_eq!(
        status,
        ArmStatus::Unavailable,
        "a cancelled query embed degrades the arm, it does not fail the search"
    );
    assert!(ranks.is_empty(), "all-or-nothing: {ranks:?}");
    assert_eq!(
        embedder.calls.load(Ordering::SeqCst),
        1,
        "only the query embed was ever started — the entry loop is never reached"
    );
}
