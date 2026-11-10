//! Identity drift on the description write-back (issue #1445): a drifted row is
//! held, never written, and only a human dismissal releases it.

use super::support::*;

fn resolved(title: &str, company: &str, url: &str) -> crate::autopilot::ResolvedIdentity {
    crate::autopilot::ResolvedIdentity {
        title: title.into(),
        company: company.into(),
        url: url.into(),
        external_id: None,
    }
}

// ── identity drift (issue #1445) ──────────────────────────────────────────────

#[test]
fn a_drifted_row_is_held_not_written_and_dismiss_releases_it() {
    let (_temp, store) = temp_store();
    let ap = create_ap(&store, "AP1", "linkedin", 0.0, "manual");
    let url = "https://www.adzuna.de/land/ad/1";
    record(
        &store,
        &ap.id,
        1,
        vec![found_job_full(url, "Rust Engineer", "Acme", 1)],
    );
    let normalized = crate::applications::normalize_job_url(url);

    let other = resolved("Cook", "Diner", "https://boards.greenhouse.io/diner/jobs/1");
    let out = store.update_found_job_descriptions(&normalized, Some("other ad text"), &other);
    assert_eq!((out.updated, out.drifted), (0, 1));
    let job = &store.list()[0].found_jobs[0];
    assert_eq!(
        job.description, None,
        "the other ad text must not be written"
    );
    assert_eq!(
        job.identity_review.as_ref().unwrap().resolved_company,
        "Diner"
    );

    assert!(store.dismiss_identity_review(&ap.id, &normalized));
    assert!(store.list()[0].found_jobs[0].identity_review.is_none());
    // Dismiss means "seen, stop flagging": the other advert text is still not
    // written onto the old row, and the drift does not re-flag.
    let out = store.update_found_job_descriptions(&normalized, Some("other ad text"), &other);
    assert_eq!((out.updated, out.drifted), (0, 0));
    let job = &store.list()[0].found_jobs[0];
    assert!(job.identity_review.is_none());
    assert_eq!(job.description, None);
}

/// A redirect to the same company and role whose ad names a Spanish requirement
/// for an Italy role: identity matches, so the text is stored verbatim.
#[test]
fn a_same_advert_stores_the_description_verbatim() {
    let (_temp, store) = temp_store();
    let ap = create_ap(&store, "AP1", "linkedin", 0.0, "manual");
    let url = "https://www.adzuna.de/land/ad/2";
    record(
        &store,
        &ap.id,
        1,
        vec![found_job_full(
            url,
            "Account Manager Italy (m/w/d)",
            "Acme GmbH",
            1,
        )],
    );
    let normalized = crate::applications::normalize_job_url(url);
    let text = "Account Manager Italy. Requirements: fluent Spanish required.";
    let same = resolved(
        "Account Manager Italy",
        "Acme",
        "https://boards.greenhouse.io/acme/jobs/5",
    );
    let out = store.update_found_job_descriptions(&normalized, Some(text), &same);
    assert_eq!((out.updated, out.drifted), (1, 0));
    assert_eq!(
        store.list()[0].found_jobs[0].description.as_deref(),
        Some(text)
    );
}

#[test]
fn a_matching_resolved_identity_cannot_release_or_write_a_held_row() {
    let (_temp, store) = temp_store();
    let ap = create_ap(&store, "AP1", "linkedin", 0.0, "manual");
    let url = "https://www.adzuna.de/land/ad/3";
    record(
        &store,
        &ap.id,
        1,
        vec![found_job_full(url, "Rust Engineer", "Acme", 1)],
    );
    let normalized = crate::applications::normalize_job_url(url);

    let other = resolved("Cook", "Diner", "https://boards.greenhouse.io/diner/jobs/1");
    store.update_found_job_descriptions(&normalized, Some("other"), &other);
    let stamped = store.list()[0].found_jobs[0]
        .identity_review
        .clone()
        .expect("held");

    // A caller asserting the ORIGINAL identity must not lift the hold or write.
    let out = store.update_found_job_descriptions(
        &normalized,
        Some("forged"),
        &resolved(
            "Rust Engineer",
            "Acme",
            "https://boards.greenhouse.io/acme/jobs/1",
        ),
    );
    assert_eq!((out.updated, out.drifted), (0, 1));
    let job = &store.list()[0].found_jobs[0];
    assert_eq!(
        job.identity_review.as_ref(),
        Some(&stamped),
        "hold survives, not re-stamped"
    );
    assert_eq!(job.description, None);
}

#[test]
fn a_dismissed_drift_is_skipped_and_a_no_evidence_resolve_changes_nothing() {
    let (_temp, store) = temp_store();
    let ap = create_ap(&store, "AP1", "linkedin", 0.0, "manual");
    let url = "https://www.adzuna.de/land/ad/4";
    record(
        &store,
        &ap.id,
        1,
        vec![found_job_full(url, "Rust Engineer", "Acme", 1)],
    );
    let normalized = crate::applications::normalize_job_url(url);

    let blank = resolved("", "", "https://example.com/careers/1");
    let out = store.update_found_job_descriptions(&normalized, Some("x"), &blank);
    assert_eq!((out.updated, out.drifted), (0, 0));
    assert_eq!(store.list()[0].found_jobs[0].description, None);

    let other = resolved("Cook", "Diner", "https://boards.greenhouse.io/diner/jobs/1");
    store.update_found_job_descriptions(&normalized, Some("t"), &other);
    assert!(store.dismiss_identity_review(&ap.id, &normalized));
    let out = store.update_found_job_descriptions(&normalized, Some("accepted"), &other);
    assert_eq!(
        (out.updated, out.drifted),
        (0, 0),
        "a dismissed drift is skipped: no write, no flag"
    );
    assert_eq!(store.list()[0].found_jobs[0].description, None);
}

#[test]
fn verdicts_are_independent_per_copy_and_each_dismissal_releases_its_own_drift() {
    let (_temp, store) = temp_store();
    let url = "https://www.adzuna.de/land/ad/9";
    let ap1 = create_ap(&store, "AP1", "linkedin", 0.0, "manual");
    let ap2 = create_ap(&store, "AP2", "indeed", 0.0, "manual");
    // Different stored titles for the same url.
    record(
        &store,
        &ap1.id,
        1,
        vec![found_job_full(url, "Rust Engineer", "Acme", 1)],
    );
    record(
        &store,
        &ap2.id,
        1,
        vec![found_job_full(url, "Cook", "Diner", 2)],
    );
    let normalized = crate::applications::normalize_job_url(url);

    // The resolved posting is the Rust Engineer advert: AP1 matches, AP2 drifts.
    let out = store.update_found_job_descriptions(
        &normalized,
        Some("rust text"),
        &resolved(
            "Rust Engineer",
            "Acme",
            "https://boards.greenhouse.io/acme/jobs/1",
        ),
    );
    assert_eq!((out.updated, out.drifted), (1, 1));
    let list = store.list();
    let by = |id: &str| list.iter().find(|a| a.id == id).unwrap().found_jobs[0].clone();
    assert_eq!(by(&ap1.id).description.as_deref(), Some("rust text"));
    assert!(by(&ap1.id).identity_review.is_none());
    assert!(by(&ap2.id).identity_review.is_some());
    assert_eq!(by(&ap2.id).description, None);

    // AP1 now drifts too (a different advert than AP2 holds), so each copy
    // holds its own drift and each dismissal releases only its own.
    store.update_found_job_descriptions(
        &normalized,
        None,
        &resolved("Cook", "Diner", "https://boards.greenhouse.io/diner/jobs/1"),
    );
    assert!(store.dismiss_identity_review(&ap1.id, &normalized));
    let list = store.list();
    let held = |id: &str| {
        list.iter().find(|a| a.id == id).unwrap().found_jobs[0]
            .identity_review
            .is_some()
    };
    assert!(!held(&ap1.id) && held(&ap2.id));
    assert!(store.dismiss_identity_review(&ap2.id, &normalized));
    assert!(store
        .list()
        .iter()
        .all(|a| a.found_jobs[0].identity_review.is_none()));
}

#[test]
fn skipped_held_and_verdict_only_passes_do_not_touch_updated_at() {
    let (_temp, store) = temp_store();
    let ap = create_ap(&store, "AP1", "linkedin", 0.0, "manual");
    let url = "https://www.adzuna.de/land/ad/10";
    record(
        &store,
        &ap.id,
        1,
        vec![found_job_full(url, "Rust Engineer", "Acme", 1)],
    );
    let normalized = crate::applications::normalize_job_url(url);
    let stamp = || store.list()[0].updated_at;

    // Verdict-only on the same advert: nothing changes.
    let before = stamp();
    std::thread::sleep(std::time::Duration::from_millis(5));
    store.update_found_job_descriptions(
        &normalized,
        None,
        &resolved(
            "Rust Engineer",
            "Acme",
            "https://boards.greenhouse.io/acme/jobs/1",
        ),
    );
    assert_eq!(stamp(), before, "Same + no description must not save");

    // NoEvidence: nothing changes.
    store.update_found_job_descriptions(
        &normalized,
        Some("x"),
        &resolved("", "", "https://e.com/1"),
    );
    assert_eq!(stamp(), before, "NoEvidence must not save");

    // First drift stamps; a repeat pass on the held row does not.
    let other = resolved("Cook", "Diner", "https://boards.greenhouse.io/diner/jobs/1");
    std::thread::sleep(std::time::Duration::from_millis(5));
    store.update_found_job_descriptions(&normalized, None, &other);
    let held_at = stamp();
    assert!(held_at > before);
    std::thread::sleep(std::time::Duration::from_millis(5));
    store.update_found_job_descriptions(&normalized, None, &other);
    assert_eq!(stamp(), held_at, "an already-held row must not be re-saved");
}
