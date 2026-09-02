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

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(f)
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
    let (ranks, status) = run_lexical_arm(&corpus(), "scanned", 3);
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
    let (ranks, _) = run_lexical_arm(&entries, "export", 3);
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
    let (ranks, status) = run_lexical_arm(&corpus(), "kubernetes", 3);
    assert!(ranks.is_empty());
    assert_eq!(
        status,
        ArmStatus::Ran,
        "zero hits is a real result, never Unavailable"
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

    let (ranks, status) = block_on(run_dense_arm(
        &store,
        &active,
        &embedder,
        "how do i export",
        &corpus(),
    ));

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

    let (ranks, status) = block_on(run_dense_arm(&store, &active, &embedder, "q", &entries));

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

    let (ranks, status) = block_on(run_dense_arm(&store, &active, &embedder, "q", &entries));

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

    let (_, status) = block_on(run_dense_arm(&store, &active, &embedder, "q", &entries));

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

    let (ranks, status) = block_on(run_dense_arm(&store, &active, &embedder, "q", &entries));

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

    let (ranks, status) = block_on(run_dense_arm(&store, &active, &embedder, "q", &corpus()));

    assert!(ranks.is_empty());
    assert_eq!(status, ArmStatus::Unavailable);
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

    let (ranks, status) = block_on(run_dense_arm(&store, &active, &embedder, "q", &corpus()));

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
