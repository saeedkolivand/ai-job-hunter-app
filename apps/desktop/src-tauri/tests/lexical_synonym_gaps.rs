//! Deterministic measurement of TWO narrow properties of hybrid search's
//! lexical arm and candidate-pool policy — this is **not** a retrieval-
//! quality or embedding benchmark. Read this doc before touching anything
//! below; the name of this file is chosen so nobody mistakes it for one.
//!
//! 1. **BM25 has no synonym handling on this corpus.** `retrieval::lexical`
//!    is SQLite FTS5 with the default `unicode61` tokenizer and no synonym
//!    layer — `sanitize_query` only quotes whitespace-split tokens (see
//!    that file's doc comment). The keyword-*scoring* kernel
//!    (`documents::keywords::SYNONYMS`, 24 alias→canonical pairs) is never
//!    consulted by the search arm. This file freezes a COPY of those 24
//!    pairs (never imports the live `SYNONYMS` directly — see the note on
//!    [`SYNONYM_PAIRS`]) and measures, per pair, whether a posting
//!    containing ONLY the alias form is found by a lexical query for the
//!    canonical form.
//!
//! 2. **The dense candidate-pool policy does not bridge that gap.**
//!    `commands::hybrid_search::dense_candidate_pool` builds its embedding
//!    candidate list FROM the lexical arm's hits whenever lexical found
//!    anything at all — never from the full eligible corpus in that case
//!    (see ADR-039). So once a distractor posting containing the canonical
//!    term exists anywhere in the corpus, an alias-only posting is excluded
//!    from the dense candidate pool even though it is part of the eligible
//!    corpus. That function (and its `PostingRow`/`DENSE_CANDIDATE_MAX`) are
//!    `private` to `commands::hybrid_search` — not `pub`/`pub(crate)` — and
//!    this task is scoped to `tests/` only (no `src/` edits), so they
//!    cannot be imported from here. [`mirrored_dense_candidate_pool`]
//!    reproduces its non-empty-`lexical_ranks` branch VERBATIM instead of
//!    calling it — see that function's doc for the drift caveat this
//!    creates and why it is tolerable for this corpus size.
//!
//! **What this file does NOT measure:** retrieval quality, ranking
//! precision/recall, or the embedding model — there is no live embedding
//! and no dense arm anywhere in this file. A previous proposal to measure
//! "hybrid finds what keyword misses" was rejected as unsound (it conflates
//! the synonym-gap question with a retrieval-quality question); do not
//! resurrect that shape here.
//!
//! Entirely deterministic — a fresh in-memory FTS5 index per case, dropped
//! immediately after. Run with
//! `cargo test --test lexical_synonym_gaps -- --nocapture` to see the
//! per-pair table.

use ajh_tauri::documents::keywords::SYNONYMS as LIVE_SYNONYMS;
use ajh_tauri::retrieval::lexical::{LexicalDoc, LexicalIndex};

// ── Frozen synonym pairs ─────────────────────────────────────────────────────

/// A COPY of `documents::keywords::SYNONYMS`, frozen rather than imported.
///
/// That table is scoring data pinned to `MATCH_FORMULA_VERSION` (changing
/// the live table changes every document's keyword set and re-scores the
/// whole corpus — see its own doc comment). Importing it here would let an
/// eval-motivated edit to this file's fixture set silently double as a
/// scoring-formula change, which is exactly backwards: this file measures
/// what the table's ABSENCE from search costs, so it must never be able to
/// move the table itself. [`frozen_synonym_pairs_match_the_live_table`]
/// below guards the other direction (this copy going stale against the real
/// table) — a failure there means resync this list by hand, deliberately.
const SYNONYM_PAIRS: &[(&str, &str)] = &[
    ("js", "javascript"),
    ("ts", "typescript"),
    ("py", "python"),
    ("golang", "go"),
    ("k8s", "kubernetes"),
    ("kube", "kubernetes"),
    ("node", "nodejs"),
    ("react.js", "react"),
    ("vue.js", "vue"),
    ("next.js", "nextjs"),
    ("nuxt.js", "nuxtjs"),
    ("psql", "postgresql"),
    ("postgres", "postgresql"),
    ("mongo", "mongodb"),
    ("tf", "tensorflow"),
    ("sklearn", "scikit-learn"),
    ("scikit", "scikit-learn"),
    ("ci/cd", "cicd"),
    ("c/c++", "cpp"),
    ("c++", "cpp"),
    ("objective-c", "objectivec"),
    ("llms", "llm"),
    ("genai", "generativeai"),
    ("gen-ai", "generativeai"),
];

/// Canonical forms that are normalised TOKENS produced by the keyword
/// kernel's own tokenizer (joined/stripped of punctuation before stemming) —
/// not text a person would type into a search box (nobody searches
/// "nextjs" or "cicd" with no punctuation at all). Flagged for the printed
/// table only: the query/index procedure below is IDENTICAL for every row
/// regardless of this flag — nothing here special-cases these to make them
/// pass or fail differently. Per the module doc's "handle that honestly"
/// instruction: the alternative to silently ignoring the distinction is
/// naming it, so a reader can tell "BM25 has no synonym table" apart from
/// "nobody would type this exact string anyway" — both are true, for
/// different rows.
const NORMALIZED_TOKEN_CANONICALS: &[&str] = &[
    "nodejs",
    "nextjs",
    "nuxtjs",
    "cicd",
    "cpp",
    "objectivec",
    "generativeai",
];

#[test]
fn frozen_synonym_pairs_match_the_live_table() {
    assert_eq!(
        SYNONYM_PAIRS, LIVE_SYNONYMS,
        "documents::keywords::SYNONYMS changed — resync SYNONYM_PAIRS above by hand \
         (deliberately not imported directly; see its doc comment)"
    );
}

// ── Row 1: does BM25 find the canonical query in an alias-only posting? ─────

/// Neutral filler for every field a case isn't exercising, chosen to contain
/// none of the 24 canonical/alias forms above — it can never accidentally
/// cause a hit.
const FILLER_TITLE: &str = "Engineer";
const FILLER_COMPANY: &str = "Acme Corp";
const FILLER_LOCATION: &str = "Remote";

/// `true` iff a fresh, single-document FTS5 index — whose only relevant text
/// is `alias` (in the description field; everything else neutral filler) —
/// is found by a lexical search for `canonical`. A fresh [`LexicalIndex`]
/// per call, exactly as `commands::hybrid_search` builds one per search (see
/// that module's doc): no shared state or corpus contamination between
/// pairs.
fn lexical_finds_canonical_in_alias_only_posting(alias: &str, canonical: &str) -> bool {
    let doc = LexicalDoc {
        id: "posting",
        title: FILLER_TITLE,
        company: FILLER_COMPANY,
        location: FILLER_LOCATION,
        description: alias,
    };
    let index = LexicalIndex::build(&[doc]).expect("build in-memory FTS5 index");
    index
        .search(canonical, 10)
        .expect("lexical search")
        .iter()
        .any(|id| id == "posting")
}

// ── Row 2: does the candidate pool bridge the gap when a distractor exists? ─

/// Mirrors `commands::hybrid_search::dense_candidate_pool`'s non-empty-
/// `lexical_ranks` branch VERBATIM. Reproduced here rather than imported
/// because that function (and its `PostingRow`/`DENSE_CANDIDATE_MAX`) are
/// private to that module and this task is scoped to `tests/` only.
///
/// **This is a mirror, not a call to production code — read this before
/// trusting it.** Verified against `src/commands/hybrid_search.rs:515-532`
/// as of this writing:
/// `if lexical_ranks.is_empty() { take from eligible } else { take from
/// lexical_ranks }`, both bounded by `DENSE_CANDIDATE_MAX = 40` (also
/// private; hardcoded below). If the real function's branch logic or that
/// constant changes, this copy goes stale SILENTLY — there is no public
/// seam to re-check it against, unlike [`frozen_synonym_pairs_match_the_live_table`]
/// above. Two things keep that tolerable rather than misleading here:
/// (a) every corpus below has exactly 2 eligible postings, far under 40, so
/// the constant's exact value cannot change the outcome asserted below —
/// only the branch CHOICE matters; (b) the branch choice itself is
/// independently pinned in-crate by
/// `dense_candidate_pool_uses_lexical_order_when_lexical_found_something`
/// (`src/commands/hybrid_search/test.rs`), so a change to the branch would
/// already fail a test in the crate before this mirror could go stale
/// unnoticed.
fn mirrored_dense_candidate_pool<'a>(
    eligible: &'a [&'a str],
    lexical_ranks: &'a [String],
) -> Vec<&'a str> {
    const DENSE_CANDIDATE_MAX: usize = 40;
    if lexical_ranks.is_empty() {
        eligible.iter().copied().take(DENSE_CANDIDATE_MAX).collect()
    } else {
        lexical_ranks
            .iter()
            .take(DENSE_CANDIDATE_MAX)
            .map(String::as_str)
            .collect()
    }
}

struct GapRow {
    alias: &'static str,
    canonical: &'static str,
    normalized_token: bool,
    lexical_hit: bool,
}

/// How many of the 24 pairs are lexical gaps, measured HERE under rusqlite's
/// bundled FTS5 build — not assumed from the earlier Python/stdlib-sqlite
/// measurement (which found 22, with `react.js`/`vue.js` as the two
/// exceptions: unicode61 splits on `.`, so BM25 already tokenizes
/// "react.js"/"vue.js" into ["react","js"]/["vue","js"] and a bare-word
/// query matches the token — no synonym table involved). If this count ever
/// moves, that means a tokenizer or rusqlite bundling change, and the
/// constant + the exact-pairs assertion below should be updated with a
/// reason, never silently edited to match.
const EXPECTED_LEXICAL_MISS_COUNT: usize = 22;

#[test]
fn bm25_has_no_synonym_handling_and_the_pool_does_not_bridge_it() {
    let rows: Vec<GapRow> = SYNONYM_PAIRS
        .iter()
        .map(|&(alias, canonical)| GapRow {
            alias,
            canonical,
            normalized_token: NORMALIZED_TOKEN_CANONICALS.contains(&canonical),
            lexical_hit: lexical_finds_canonical_in_alias_only_posting(alias, canonical),
        })
        .collect();

    println!("\n=== lexical_synonym_gaps: BM25 synonym-gap + candidate-pool measurement ===");
    println!(
        "{:<14} {:<16} {:<7} {:<7}  note",
        "alias", "canonical", "lexical", "normtok"
    );
    for row in &rows {
        println!(
            "{:<14} {:<16} {:<7} {:<7}  {}",
            row.alias,
            row.canonical,
            if row.lexical_hit { "HIT" } else { "MISS" },
            if row.normalized_token { "yes" } else { "" },
            if row.normalized_token {
                "canonical is a normalised token, not natural query text"
            } else {
                ""
            }
        );
    }

    let miss_count = rows.iter().filter(|r| !r.lexical_hit).count();
    println!(
        "\n{miss_count} of {} pairs are lexical gaps (BM25 finds none of them for the canonical \
         query)\n",
        rows.len()
    );

    // Literal, not derived-vs-derived: compared against a hand-set constant,
    // not against another computed value.
    assert_eq!(
        miss_count,
        EXPECTED_LEXICAL_MISS_COUNT,
        "measured {miss_count} lexical gaps of {} pairs, expected {EXPECTED_LEXICAL_MISS_COUNT} \
         — see the printed table above for which pair(s) moved and why",
        rows.len()
    );

    // The two pairs measured as non-gaps must be EXACTLY the ones the module
    // doc names — not some other pair that happens to also hit for an
    // unrelated reason.
    let hits: Vec<&str> = rows
        .iter()
        .filter(|r| r.lexical_hit)
        .map(|r| r.alias)
        .collect();
    assert_eq!(
        hits,
        vec!["react.js", "vue.js"],
        "the exact pairs BM25 finds despite no synonym table changed — expected only react.js \
         and vue.js (unicode61 splits on '.'); update the module doc if this is a deliberate, \
         understood change"
    );

    // ── Row 2: candidate-pool exclusion, for every CONFIRMED lexical gap ────
    for row in rows.iter().filter(|r| !r.lexical_hit) {
        let alias_id = "alias";
        let distractor_id = "distractor";
        let alias_doc = LexicalDoc {
            id: alias_id,
            title: FILLER_TITLE,
            company: FILLER_COMPANY,
            location: FILLER_LOCATION,
            description: row.alias,
        };
        let distractor_doc = LexicalDoc {
            id: distractor_id,
            // The canonical term lives in the title (BM25's highest-weighted
            // column) so the distractor is unambiguously the strongest
            // lexical hit — the point being tested is exclusion from the
            // POOL, not a close ranking call.
            title: row.canonical,
            company: FILLER_COMPANY,
            location: FILLER_LOCATION,
            description: "Distractor posting mentioning the canonical term.",
        };
        let index = LexicalIndex::build(&[alias_doc, distractor_doc])
            .expect("build mixed-corpus in-memory FTS5 index");
        let lexical_ranks = index
            .search(row.canonical, 10)
            .expect("lexical search over mixed corpus");

        // The distractor must actually be found lexically, or this row
        // proves nothing about the pool policy — it would just be a second
        // copy of the row-1 miss.
        assert!(
            lexical_ranks.iter().any(|id| id == distractor_id),
            "{}: distractor posting (title={:?}) was not found by lexical search for {:?} — \
             fixture is broken, this pair cannot test pool exclusion",
            row.alias,
            row.canonical,
            row.canonical
        );

        let eligible = [alias_id, distractor_id];
        assert!(
            eligible.contains(&alias_id),
            "sanity: the alias posting must be part of the eligible corpus for this assertion to \
             mean anything"
        );

        let pool = mirrored_dense_candidate_pool(&eligible, &lexical_ranks);
        assert!(
            !pool.contains(&alias_id),
            "{}: alias-only posting appeared in the dense candidate pool even though it is part \
             of the eligible corpus and lexical only found the distractor — the pool-exclusion \
             property (ADR-039) did not hold for this pair",
            row.alias
        );
        assert!(
            pool.contains(&distractor_id),
            "{}: distractor unexpectedly absent from its own candidate pool",
            row.alias
        );
    }
}
