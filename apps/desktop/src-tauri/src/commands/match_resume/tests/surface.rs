use super::*;

// ── the metric-label contract: one pipeline per "Match %" ────────────

/// Both surfaces that render a "Match %" must feed `score_one` the SAME
/// pre-processing. `translate_if_needed` runs BEFORE keyword extraction and
/// BEFORE the embed, so turning it off for one surface flips
/// `languages_align` on a cross-language pair — collapsing coverage to
/// language-neutral tech tokens and embedding a cross-lingual cosine. The
/// same job would then show two materially different percentages depending
/// on which screen the user is looking at.
#[test]
fn every_match_percent_surface_runs_the_same_pre_processing() {
    assert!(
        MatchSurface::JobsPage.translates(),
        "the in-app path has always translated"
    );
    assert_eq!(
        MatchSurface::Autopilot.translates(),
        MatchSurface::JobsPage.translates(),
        "the Autopilot re-rank renders its number under the same label as the Jobs page, \
         so it must run the same pipeline — translation is cloud-excluded (local providers \
         only), cached per job id, and bounded by the caller's top-N, so there is no cost \
         argument that survives the divergence"
    );
    assert!(
        !MatchSurface::Extension.translates(),
        "the extension's zero-egress guarantee is structural: it must never reach the \
         provider layer, and it never shows a combined number"
    );
    assert_eq!(
        MatchSurface::JobAdText.translates(),
        MatchSurface::JobsPage.translates(),
        "the Score tab's ad-hoc text surface renders under the SAME 'Match' label as the \
         Jobs page and runs INSIDE the app against a user-owned résumé (not the untrusted \
         browser bridge), so it has no zero-egress obligation and must run the same pipeline \
         — unlike Extension, which is the one deliberate exception"
    );
}

/// The résumé language is resolved from ONE source — the persisted
/// (nullable) `DocumentRecord.locale`, falling back to `"en"`. An Autopilot
/// snapshot has no persisted locale, so it must land on the same fallback a
/// locale-less Jobs-page document does; detecting it on one surface only
/// would be the same divergence in the other direction.
#[test]
fn the_autopilot_resume_snapshot_resolves_its_language_like_a_jobs_page_document() {
    let german = "Erfahrener Softwareentwickler mit Kubernetes, Rust und Postgres, \
                  verantwortlich für den Aufbau verteilter Systeme.";
    let jobs_page = DocumentRecord {
        id: "doc-1".into(),
        title: String::new(),
        name: String::new(),
        locale: None, // `documents_add`'s `locale` is optional
        text: german.to_string(),
        pages: None,
        created_at: 0,
        indexed: false,
        is_default: false,
        keywords_json: None,
    };
    let autopilot = autopilot_resume_record(german);

    assert_eq!(
        autopilot.locale, jobs_page.locale,
        "same (absent) locale source on both surfaces"
    );
    assert_eq!(
        resume_target_lang(&autopilot),
        resume_target_lang(&jobs_page),
        "…so both resolve the same translation target and the same languages_align input"
    );
    assert_eq!(autopilot.text, jobs_page.text);
    assert_eq!(
        autopilot.keywords_json, None,
        "no cached token list for a raw snapshot — score_one live-extracts, its documented \
         fallback, from the identical text"
    );
}

/// A résumé SNAPSHOT has no `documents` row, so its vector must not enter
/// the document index (nothing would ever delete it, and the Embeddings
/// panel counts every row there).
#[test]
fn only_a_real_document_resume_is_written_to_the_document_vector_index() {
    assert_eq!(
        MatchSurface::JobsPage.resume_vector_home(),
        ResumeVectorHome::DocumentIndex
    );
    assert_eq!(
        MatchSurface::Extension.resume_vector_home(),
        ResumeVectorHome::DocumentIndex
    );
    assert_eq!(
        MatchSurface::JobAdText.resume_vector_home(),
        ResumeVectorHome::DocumentIndex,
        "the Score tab scores a real stored résumé, not a text snapshot"
    );
    assert_eq!(
        MatchSurface::Autopilot.resume_vector_home(),
        ResumeVectorHome::EphemeralCache,
        "the Autopilot résumé is a content-addressed snapshot: its vector belongs in the \
         TTL-pruned posting-vector cache, never in the document index"
    );
    assert!(
        crate::documents::is_synthetic_scoring_id(&autopilot_resume_id("any résumé")),
        "…and its id is one the document index refuses outright"
    );
}
