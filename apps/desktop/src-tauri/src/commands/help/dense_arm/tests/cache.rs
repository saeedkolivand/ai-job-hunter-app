use super::*;

// ── Dense arm ────────────────────────────────────────────────────────────────

#[test]
fn a_failed_query_embed_is_unavailable_and_embeds_no_entries() {
    let (_dir, store, active) = fixture();
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
    let (_dir, store, active) = fixture();
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
    let (_dir, store, active) = fixture();
    let entries = corpus();
    // Pre-seed every entry, exactly as a previous run would have.
    seed(&store, &active, &entries, &[0.0, 1.0]);
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
    let (_dir, store, active) = fixture();
    let mut entries = corpus();
    seed(&store, &active, &entries, &[0.0, 1.0]);
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
    let (_dir, store, active) = fixture();
    let old_space = cfg("openai", "text-embedding-3-small");
    let entries = corpus();
    seed(&store, &old_space, &entries, &[0.0, 1.0]);
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
    let (_dir, store, active) = fixture();
    let embedder = CallScript(AtomicUsize::new(0), active.clone(), |i| {
        (i == 0).then(|| vec![1.0, 0.0])
    });

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
    let (_dir, store, active) = fixture();
    // Query + entry 1 succeed, entry 2 fails, entry 3 succeeds.
    let embedder = CallScript(AtomicUsize::new(0), active.clone(), |i| {
        (i != 2).then(|| vec![1.0, 0.0])
    });
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
    let (_dir, store, active) = fixture();
    // Query + entries 1 and 3 embed normally; entry 2 comes back all zeros.
    // Call 0 is the query, so call 2 is the SECOND entry.
    let embedder = CallScript(AtomicUsize::new(0), active.clone(), |i| {
        Some(if i == 2 {
            vec![0.0, 0.0]
        } else {
            vec![1.0, 0.0]
        })
    });
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
    let (_dir, store, active) = fixture();
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
    let (_dir, store, active) = fixture();
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
    let (_dir, store, active) = fixture();
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
    let (_dir, store, active) = fixture();
    let many: Vec<HelpSearchRequestEntry> = (0..HELP_EMBED_MISSES_MAX + 5)
        .map(|i| entry(&format!("s.e{i}"), "title", &format!("body number {i}")))
        .collect();
    seed(&store, &active, &many, &[0.0, 1.0]);
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
    let (_dir, store, active) = fixture();
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
