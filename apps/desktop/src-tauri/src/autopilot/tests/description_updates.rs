//! `update_found_job_descriptions` (issue #1106 part b): a manual text correction reaches
//! every matching row, and re-derives only what is description-scoped.

use super::support::*;

// ── update_found_job_descriptions (issue #1106 part b) ────────────────────────

#[test]
fn update_found_job_descriptions_patches_the_matching_row() {
    let (_temp, store) = temp_store();
    let ap = create_ap(&store, "AP1", "linkedin", 0.0, "manual");
    record(
        &store,
        &ap.id,
        1,
        vec![found_job("https://boards.example.com/jobs/1", 1)],
    );

    let normalized = crate::applications::normalize_job_url("https://boards.example.com/jobs/1");
    let updated = store.update_found_job_descriptions(&normalized, "corrected text");
    assert_eq!(updated, 1, "exactly one row must be patched");

    let list = store.list();
    assert_eq!(
        list[0].found_jobs[0].description.as_deref(),
        Some("corrected text")
    );
}

#[test]
fn update_found_job_descriptions_returns_zero_when_no_url_matches() {
    let (_temp, store) = temp_store();
    let ap = create_ap(&store, "AP1", "linkedin", 0.0, "manual");
    record(
        &store,
        &ap.id,
        1,
        vec![found_job("https://boards.example.com/jobs/1", 1)],
    );

    let normalized = crate::applications::normalize_job_url("https://nowhere.example.com/x");
    let updated = store.update_found_job_descriptions(&normalized, "ignored");
    assert_eq!(updated, 0, "an unmatched url must patch nothing");
    assert_eq!(
        store.list()[0].found_jobs[0].description,
        None,
        "the unrelated row must be untouched on a miss"
    );
}

/// The same posting can legitimately surface under more than one autopilot
/// (two separate searches both matched it) — every row must update, not just
/// the first record iterated.
#[test]
fn update_found_job_descriptions_updates_every_matching_row_across_autopilots() {
    let (_temp, store) = temp_store();
    let shared_url = "https://boards.example.com/jobs/shared";

    let ap1 = create_ap(&store, "AP1", "linkedin", 0.0, "manual");
    let ap2 = create_ap(&store, "AP2", "indeed", 0.0, "manual");
    record(&store, &ap1.id, 1, vec![found_job(shared_url, 1)]);
    record(&store, &ap2.id, 1, vec![found_job(shared_url, 2)]);

    let normalized = crate::applications::normalize_job_url(shared_url);
    let updated = store.update_found_job_descriptions(&normalized, "shared correction");
    assert_eq!(
        updated, 2,
        "both autopilots' rows for the same url must update, not just the first"
    );
    for ap in store.list() {
        assert_eq!(
            ap.found_jobs[0].description.as_deref(),
            Some("shared correction"),
            "every matching row across every autopilot must be patched"
        );
    }
}

// ── update_found_job_descriptions recomputes description-dependent trust but
// leaves score_provisional untouched (issue #1106 / #1106 shared-seam fix) ──
// `score_provisional` describes the `score` field, which a manual text
// correction deliberately does NOT recompute (see the doc comment on
// `update_found_job_descriptions`) — so the flag must survive unchanged here,
// only `trust` (genuinely description-scoped) may react.

#[test]
fn update_found_job_descriptions_clears_stale_trust_but_leaves_score_provisional_untouched() {
    let (_temp, store) = temp_store();
    let ap = create_ap(&store, "AP1", "linkedin", 0.0, "manual");

    // `company_matches_host("Acme", "acme.com")` is true, so the ONLY trust
    // flag in play is the description-driven one — isolates the assertion
    // below to the thing this fix changes.
    let url = "https://acme.com/careers/1";
    let empty_desc_trust = crate::scraping::trust::assess_trust(url, "Acme", "");
    assert!(
        empty_desc_trust
            .flags
            .contains(&crate::scraping::trust::TrustFlag::DescriptionUnavailable),
        "seed sanity check: an empty description must carry DescriptionUnavailable"
    );

    let mut seeded = found_job_full(url, "Rust Engineer", "Acme", 1);
    seeded.score = Some(70.0);
    // Mirrors `build_found_job`'s `no_jd_text`-driven provisional marker for
    // a title-only posting (empty description, no requirements).
    seeded.score_provisional = true;
    seeded.trust = Some(empty_desc_trust);
    record(&store, &ap.id, 1, vec![seeded]);

    let normalized = crate::applications::normalize_job_url(url);
    let full_description = "We are looking for a Senior Rust Engineer to build our \
         distributed systems platform. You will own the async runtime, mentor \
         junior engineers, and ship production services used by millions of \
         users daily.";
    let updated = store.update_found_job_descriptions(&normalized, full_description);
    assert_eq!(updated, 1);

    let job = &store.list()[0].found_jobs[0];
    let trust = job.trust.as_ref().expect("trust must still be set");
    assert!(
        !trust
            .flags
            .contains(&crate::scraping::trust::TrustFlag::DescriptionUnavailable),
        "a real, substantial description must clear the stale \
         DescriptionUnavailable flag, not keep showing a 'no description' \
         badge next to the now-visible full text"
    );
    assert_eq!(
        trust.level,
        crate::scraping::trust::TrustLevel::High,
        "clearing the only flag must read back as a higher trust level"
    );
    assert!(
        job.score_provisional,
        "score_provisional describes `score`, which a manual description \
         correction deliberately does not recompute — it must survive \
         unchanged (still true) until an actual autopilot run re-scores \
         the corrected content, never flip to false just because the \
         description changed"
    );
}
