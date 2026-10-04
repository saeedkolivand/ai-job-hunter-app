//! The scoring entry points that wrap `score_one` for one surface each — the
//! extension's keyword-only check, the Score tab's ad-hoc job-ad text, and the
//! headless Autopilot re-rank — plus the résumé-side helpers they share. Split out
//! of `commands/match_resume.rs` for R8 (issue #1280); `match_resume.rs` re-exports
//! the crate-visible items, so each keeps its `commands::match_resume::<name>` path.

use serde_json::Value;
use tauri::AppHandle;

use super::score::{score_one, AppScoreIo, MatchSurface};
use crate::documents::{sha256_hex, DocumentRecord, DocumentStore, EmbedBudget, EmbeddingConfig};

/// Ad-hoc, KEYWORD-ONLY scoring entry point for `extension_bridge::match_live`
/// (the browser extension's "Check fit" button + its `import.result.matchScore`
/// fill) — a thin forwarding wrapper around [`score_one`], NOT a new scoring
/// path: every existing `match_resume` caller is untouched, and this adds no
/// new branch to `score_one` itself. `job_id` here is a synthetic per-URL cache
/// key (not a real `PostingsCache` id) — the caller is responsible for
/// deriving it (e.g. a hash of the normalized job url) so repeat calls for the
/// same page hit the SAME self-invalidating `match_scores` row `score_one`
/// already maintains (formula version / semantic bit / job-text hash — see
/// its cache-key doc). `job_text` is required (not `Option`) because the
/// caller always has JD text in hand by construction (a browser DOM parse,
/// never a `PostingsCache` miss).
///
/// Deliberately NO `semantic_enabled` parameter (unlike the removed
/// `score_adhoc`): semantic scoring is hardcoded OFF below, not
/// caller-configurable, and this NEVER translates ([`MatchSurface::Extension`]
/// to [`score_one`]) — the extension bridge has no channel to the app's
/// semantic-scoring setting (see `extension_bridge::match_live`'s module doc)
/// and a CLI-agent provider configured as "local" still performs cloud egress
/// despite `ProviderId::is_local()`, so the zero-egress guarantee for this
/// entry point must be structural (no flag to flip), not a default. Trade-off:
/// a foreign-language job posting is scored keyword-only against its RAW
/// (untranslated) text — an accepted accuracy cost for that guarantee.
pub(crate) async fn score_adhoc_keyword_only(
    app: &AppHandle,
    store: &DocumentStore,
    resume: &DocumentRecord,
    resume_raw_keywords: Option<&[String]>,
    active: &EmbeddingConfig,
    job_id: &str,
    job_text: String,
) -> Value {
    score_one(
        &AppScoreIo(app),
        store,
        resume,
        resume_raw_keywords,
        active,
        job_id,
        Some(job_text),
        0, // semantic_enabled hardcoded OFF — never caller-configurable
        // Never translates: this entry point must not reach the provider layer.
        MatchSurface::Extension,
        None, // keyword-only: there is no round-trip to budget
    )
    .await
}

/// Content-addressed cache identity for the Score tab's ad-hoc job-ad TEXT
/// (`JobAdView`'s "Score" sub-tab — see [`score_resume_against_text`]).
/// Callers hash the PRE-PROCESSED blob ([`job_ad_text_blob`]), never the raw
/// payload, so two differently-formatted raw postings that reduce to the same
/// plain text share one row — mirroring [`autopilot_resume_id`]'s
/// content-addressing discipline. Prefixed so it can never collide with a real
/// `PostingsCache` id (which never carries a `:`), matching the
/// `adhoc:`/`autopilot:`/`autopilot-resume:` namespace convention.
///
/// Repeated opens of the SAME posting reuse the SAME `match_scores` SQLite
/// row for that row's whole TTL — but that reuse is not unconditionally
/// "free" for a foreign-language posting: [`score_one`] always runs
/// `translate_if_needed` BEFORE the cache lookup, and that call's OWN
/// memoization ([`crate::commands::translation::TranslationCache`]) is an
/// in-memory map that resets on every process restart and clears wholesale at
/// its entry cap — so re-opening the same foreign-language posting after a
/// restart (or past that cap) still pays a fresh local-model translation
/// completion, even though the eventual score comes from the SQLite cache
/// underneath it.
pub(crate) fn job_ad_text_id(job_text: &str) -> String {
    format!("job-ad-text:{}", sha256_hex(job_text))
}

/// The Score tab's ad-hoc pre-processing: run the description through the
/// SAME blob builder [`posting_to_text`] runs on the Jobs page
/// ([`crate::documents::keywords::posting_text_blob`]), so markdown links and
/// bare URLs are stripped here too instead of leaking into the keyword set as
/// JD vocabulary — aggregator postings routinely carry markdown
/// (`html_to_markdown` in the Adzuna/freehire scrapers), so an unprocessed
/// `[Apply now](https://acme.example.com/jobs)` would otherwise inflate this
/// surface's keyword-coverage DENOMINATOR with `https`/`acme`/`example`/`com`
/// — tokens no résumé can ever contain — while the Jobs page strips them to
/// nothing. Called with an empty title and no requirements — the ONE
/// compositional axis [`MatchSurface`]'s doc names: `JobAdView` never has
/// either, only the description (`jobDesc: string`). Pure — directly
/// testable against [`posting_to_text`]'s own output for the identical
/// description; see
/// `job_ad_text_blob_matches_posting_to_text_for_the_same_markdown_description`.
pub(super) fn job_ad_text_blob(description: &str) -> Option<String> {
    crate::documents::keywords::posting_text_blob("", Some(description), None)
}

/// Score a stored résumé against arbitrary job-ad TEXT — the Score tab's entry
/// point (`JobAdView`'s "Score" sub-tab). `TailorFlow` receives an
/// `Application` / `AutopilotFoundJob`, neither of which carries a
/// `PostingsCache` id [`match_resume`] needs, and that cache is RAM-only and
/// deliberately transient (discovery is transient by design), so a saved
/// application could never have had an entry anyway — the JD text `JobAdView`
/// already holds (`jobDesc: string`) is the honest input.
///
/// A thin forwarding wrapper around [`score_one`], NOT a new scoring path — no
/// new branch is added to the kernel and every existing caller is untouched.
/// `semantic_enabled` is the caller-supplied cache-key bit
/// ([`semantic_enabled_bit`] of `MatchTextRequest.semanticScoringEnabled`) —
/// the renderer reads the SAME `useSemanticScoring()` preference the Jobs
/// page does and threads it through, so this surface is no longer hardcoded
/// keyword-only.
///
/// `job_text` is run through [`job_ad_text_blob`] BEFORE it is hashed for
/// [`job_ad_text_id`] and before scoring — see that function's doc for why.
///
/// UNLIKE [`score_adhoc_keyword_only`] ([`MatchSurface::Extension`], whose
/// zero-egress guarantee is structural because the caller is an untrusted
/// browser bridge), this surface runs inside the app against a user-owned,
/// already-stored résumé — there is no zero-egress obligation to uphold, so it
/// deliberately DOES translate ([`MatchSurface::JobAdText`], not `Extension`),
/// and opting it into the SAME semantic-scoring preference the Jobs page
/// reads carries no extra egress risk either. See [`MatchSurface`]'s own doc
/// for the honest parity claim (identical PRE-PROCESSED text, same
/// `semantic_enabled` bit) and the one axis — composition (no title/
/// requirements here) — that still legitimately diverges from the Jobs page.
pub(crate) async fn score_resume_against_text(
    app: &AppHandle,
    store: &DocumentStore,
    resume: &DocumentRecord,
    resume_raw_keywords: Option<&[String]>,
    active: &EmbeddingConfig,
    job_text: String,
    semantic_enabled: i64,
) -> Value {
    let job_text = job_ad_text_blob(&job_text);
    let job_id = job_ad_text_id(job_text.as_deref().unwrap_or(""));
    score_one(
        &AppScoreIo(app),
        store,
        resume,
        resume_raw_keywords,
        active,
        &job_id,
        job_text,
        semantic_enabled,
        MatchSurface::JobAdText,
        None, // user-initiated: not charged against the unattended daily ceiling
    )
    .await
}

/// Content-addressed cache identity for an Autopilot's résumé snapshot.
///
/// The Autopilot record persists `resume_text` (a raw string copied at setup
/// time), not a `DocumentRecord` id, so the semantic path needs a stable id for
/// its `posting_vectors` / `match_scores` rows. Hashing the text makes it
/// **self-invalidating**: editing the autopilot's résumé yields a different id,
/// so a stale résumé vector can never be scored against — the same discipline
/// `posting_vectors.text_hash` uses.
///
/// The `autopilot-resume:` namespace prefix does two jobs. It marks the id as
/// synthetic, so `DocumentStore::upsert_vector` REFUSES it (see
/// `documents::is_synthetic_scoring_id`) and the document index can never
/// acquire a row nothing ever deletes. And it separates this key space from the
/// posting keys (`autopilot:<hash of canonical_job_key>`), which now share the
/// `posting_vectors` table with it.
///
/// `pub(crate)` so `commands::autopilot`'s cache-reuse test can assert against
/// the REAL identity instead of a hand-retyped mirror of this format string.
pub(crate) fn autopilot_resume_id(resume_text: &str) -> String {
    format!("autopilot-resume:{}", sha256_hex(resume_text))
}

/// The synthetic [`DocumentRecord`] the Autopilot re-rank scores with — an
/// Autopilot stores résumé TEXT, not a document reference.
///
/// Only the four fields `score_one` reads are meaningful:
/// - `id` — [`autopilot_resume_id`], the content-addressed cache identity;
/// - `text` — the résumé itself;
/// - `locale` — **`None`, deliberately**: the Jobs page reads the persisted
///   (nullable) `DocumentRecord.locale` and falls back to `"en"`
///   ([`resume_target_lang`]), and a snapshot has no persisted locale, so
///   `None` is the SAME source resolving the SAME way. Detecting the language
///   here instead would make the two surfaces disagree about a non-English
///   résumé — a parity break in the opposite direction from a missing
///   translate step. (Detect-and-backfill onto the document row would be
///   better behaviour for both surfaces; that is a separate change, not
///   something to smuggle into one of them.)
/// - `keywords_json: None` — no cached token list, so `score_one` live-extracts
///   (its documented fallback).
pub(crate) fn autopilot_resume_record(resume_text: &str) -> DocumentRecord {
    DocumentRecord {
        id: autopilot_resume_id(resume_text),
        title: String::new(),
        name: String::new(),
        locale: None,
        text: resume_text.to_string(),
        pages: None,
        created_at: 0,
        indexed: false,
        is_default: false,
        keywords_json: None,
    }
}

/// SEMANTIC (combined) scoring entry point for the headless Autopilot re-rank —
/// a thin forwarding wrapper around [`score_one`], NOT a second scoring path:
/// no new branch is added to `score_one`, so Autopilot and the Jobs page share
/// one kernel, `languages_align` included. That inclusion is the point: the
/// Autopilot's previous `coverage_score` path stemmed BOTH sides with the JD
/// stemmer unconditionally, which mangles language-neutral tokens on a
/// cross-language résumé↔posting pair; routing through `score_one` closes that
/// known divergence (ADR-020 addendum).
///
/// `job_id` is a synthetic per-job cache key the caller derives (see
/// `commands::autopilot::autopilot_job_id`) — Autopilot postings never enter
/// `PostingsCache`, so there is no real posting id to use.
///
/// The résumé is wrapped by [`autopilot_resume_record`] (see its doc for why
/// every field is what it is).
///
/// [`MatchSurface::Autopilot`] means the FULL pre-processing pipeline runs here,
/// exactly as on the Jobs page — translation included. That is not a cost
/// decision to re-litigate per surface: the number is rendered under the same
/// "Match %" label, and translation is cloud-excluded (local providers only, so
/// it cannot incur an API cost), cached per job id for the session, and bounded
/// by the caller's top-N ceiling.
///
/// `budget` is the headless run's share of the shared per-provider daily
/// ceiling. It is charged inside the kernel, once per embed that actually
/// happens (see [`score_one`]) — a fully-cached job costs nothing, and a job
/// that has to embed BOTH the résumé snapshot and the posting costs two.
pub(crate) async fn score_autopilot_semantic(
    app: &AppHandle,
    store: &DocumentStore,
    resume_text: &str,
    active: &EmbeddingConfig,
    job_id: &str,
    job_text: String,
    budget: &dyn EmbedBudget,
) -> Value {
    let resume = autopilot_resume_record(resume_text);
    score_one(
        &AppScoreIo(app),
        store,
        &resume,
        None, // no cached keyword list for a raw résumé snapshot — live-extract
        active,
        job_id,
        Some(job_text),
        1, // semantic_enabled: this entry point exists only for the semantic re-rank
        MatchSurface::Autopilot,
        Some(budget),
    )
    .await
}

/// Parse the résumé's cached normalized keywords (`keywords_json`) into a token
/// list. Absent OR corrupt JSON → `None`, which makes [`score_one`] fall back to
/// live extraction from `resume.text` (the legacy behaviour). `pub(crate)` so
/// `extension_bridge::match_live` reuses the SAME fallback rule instead of
/// re-deriving it.
pub(crate) fn parse_resume_keywords(resume: &DocumentRecord) -> Option<Vec<String>> {
    resume
        .keywords_json
        .as_deref()
        .and_then(|j| serde_json::from_str::<Vec<String>>(j).ok())
}
