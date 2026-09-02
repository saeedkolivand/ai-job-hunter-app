//! Deterministic measurement of ONE narrow property of hybrid search's
//! lexical arm — this is **not** a retrieval-quality or embedding
//! benchmark. Read this doc before touching anything below; the name of
//! this file is chosen so nobody mistakes it for one.
//!
//! **What this measures:** BM25 has no synonym handling on this corpus.
//! `retrieval::lexical` is SQLite FTS5 with the default `unicode61`
//! tokenizer and no synonym layer — `sanitize_query` only quotes
//! whitespace-split tokens (see that file's doc comment). The keyword-
//! *scoring* kernel (`documents::keywords::SYNONYMS`, 24 alias→canonical
//! pairs) is never consulted by the search arm. This file freezes a COPY of
//! those 24 pairs (never imports the live `SYNONYMS` directly — see the
//! note on [`SYNONYM_PAIRS`]) and measures, per pair, whether a posting
//! containing ONLY the alias form is found by a lexical query for the
//! canonical form.
//!
//! **What moved out (PR #1091 review):** the companion measurement — that
//! the dense candidate-pool policy does not bridge this gap once a
//! distractor posting contains the canonical term — used to live here as a
//! hand-mirrored copy of `commands::hybrid_search::dense_candidate_pool`'s
//! logic, because that function is private to its module and this file (an
//! external integration test in a separate crate) could not call it
//! directly. That mirror was flagged Major: it had no seam to check it
//! against the real function, so it could drift silently. It now lives in
//! `src/commands/hybrid_search/test.rs` — an in-crate test module that
//! already has access to the real private `dense_candidate_pool`,
//! `PostingRow`, and `to_lexical_doc` via `use super::*;` — and calls the
//! REAL function instead of a copy. See
//! `dense_candidate_pool_excludes_alias_only_posting_when_a_distractor_hits`
//! there for that half of the measurement.
//!
//! **What this file does NOT measure:** retrieval quality, ranking
//! precision/recall, the embedding model, or the candidate-pool policy (see
//! above, now in-crate) — there is no live embedding and no dense arm
//! anywhere in this file. A previous proposal to measure "hybrid finds what
//! keyword misses" was rejected as unsound (it conflates the synonym-gap
//! question with a retrieval-quality question); do not resurrect that shape
//! here.
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
///
/// A second, independent copy (`POOL_SYNONYM_PAIRS`) lives in
/// `src/commands/hybrid_search/test.rs` for the candidate-pool half of this
/// measurement, for the same non-import reason — the two are kept in
/// lockstep by hand, each guarded against the live table separately.
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
        "documents::keywords::SYNONYMS changed — resync SYNONYM_PAIRS above (and \
         POOL_SYNONYM_PAIRS in src/commands/hybrid_search/test.rs) by hand"
    );
}

// ── Row 1: does BM25 find the canonical query in an alias-only posting? ─────

/// Neutral filler for every field a case isn't exercising, chosen to contain
/// none of the 24 canonical/alias forms above — it can never accidentally
/// cause a hit.
const FILLER_TITLE: &str = "Engineer";
const FILLER_COMPANY: &str = "Acme Corp";
const FILLER_LOCATION: &str = "Remote";

/// `Ok(true)`/`Ok(false)` for hit/miss of a lexical search for `canonical`
/// against a fresh, single-document FTS5 index whose only relevant text is
/// `alias` (in the description field; everything else neutral filler).
/// `Err` on a genuine build/search failure — returned rather than panicking
/// (`.expect`) so a single bad pair cannot abort the `.map().collect()`
/// below before the diagnostic table has a chance to print for every OTHER
/// pair (the earlier version of this function did exactly that).
///
/// A fresh [`LexicalIndex`] per call, exactly as `commands::hybrid_search`
/// builds one per search (see that module's doc): no shared state or
/// corpus contamination between pairs.
fn lexical_finds_canonical_in_alias_only_posting(
    alias: &str,
    canonical: &str,
) -> Result<bool, String> {
    let doc = LexicalDoc {
        id: "posting",
        title: FILLER_TITLE,
        company: FILLER_COMPANY,
        location: FILLER_LOCATION,
        description: alias,
    };
    let index = LexicalIndex::build(&[doc]).map_err(|e| format!("build index: {e}"))?;
    let hits = index
        .search(canonical, 10)
        .map_err(|e| format!("search: {e}"))?;
    Ok(hits.iter().any(|id| id == "posting"))
}

struct GapRow {
    alias: &'static str,
    canonical: &'static str,
    normalized_token: bool,
    lexical_hit: Result<bool, String>,
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
fn bm25_has_no_synonym_handling_on_this_corpus() {
    // `.map().collect()` over `Result`-returning per-row work, NOT
    // `.expect()` inside the closure: a single pair erroring must not abort
    // construction before the table below has a chance to print every OTHER
    // row's result.
    let rows: Vec<GapRow> = SYNONYM_PAIRS
        .iter()
        .map(|&(alias, canonical)| GapRow {
            alias,
            canonical,
            normalized_token: NORMALIZED_TOKEN_CANONICALS.contains(&canonical),
            lexical_hit: lexical_finds_canonical_in_alias_only_posting(alias, canonical),
        })
        .collect();

    println!("\n=== lexical_synonym_gaps: BM25 synonym-gap measurement ===");
    println!(
        "{:<14} {:<16} {:<7} {:<7}  note",
        "alias", "canonical", "lexical", "normtok"
    );
    for row in &rows {
        let (status, error_note) = match &row.lexical_hit {
            Ok(true) => ("HIT", String::new()),
            Ok(false) => ("MISS", String::new()),
            Err(e) => ("ERROR", format!("({e})")),
        };
        let note = if row.normalized_token {
            format!("canonical is a normalised token, not natural query text {error_note}")
        } else {
            error_note
        };
        println!(
            "{:<14} {:<16} {:<7} {:<7}  {}",
            row.alias,
            row.canonical,
            status,
            if row.normalized_token { "yes" } else { "" },
            note
        );
    }

    // The table above is now printed for every row regardless of outcome —
    // ONLY NOW do we assert, so a build/search failure on one pair never
    // hides the other 23 results.
    let errored: Vec<&str> = rows
        .iter()
        .filter(|r| r.lexical_hit.is_err())
        .map(|r| r.alias)
        .collect();
    assert!(
        errored.is_empty(),
        "these pairs errored building/searching the in-memory FTS5 index (see the table above \
         for the message): {errored:?}"
    );

    let miss_count = rows
        .iter()
        .filter(|r| matches!(r.lexical_hit, Ok(false)))
        .count();
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

    // The pairs measured as non-gaps must be EXACTLY the ones the module
    // doc names — not some other pair that happens to also hit for an
    // unrelated reason.
    let hits: Vec<&str> = rows
        .iter()
        .filter(|r| matches!(r.lexical_hit, Ok(true)))
        .map(|r| r.alias)
        .collect();
    assert_eq!(
        hits,
        vec!["react.js", "vue.js"],
        "the exact pairs BM25 finds despite no synonym table changed — expected only react.js \
         and vue.js (unicode61 splits on '.'); update the module doc if this is a deliberate, \
         understood change"
    );
}
