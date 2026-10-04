use super::*;

// ── match_resume_text's pure precondition (resolve_resume_and_text) ──────────

/// The résumé-not-found error must be returned BEFORE any clamp/cache work —
/// mirrors `match_resume`'s own resume-not-found shape and the errors-never-
/// cached invariant.
#[test]
fn resolve_resume_and_text_reports_resume_not_found() {
    let (_dir, store) = scoring_store();
    let err = resolve_resume_and_text(&store, "missing-resume", "some job text".into())
        .expect_err("no such resume must be an error, not a silent default");
    assert_eq!(err["error"], "resume not found: missing-resume");
}

/// Job text over [`MAX_JOB_DESCRIPTION_BYTES`] must be clamped, not rejected —
/// mirrors `resume_trim_suggestions`'s convention (an advisory/estimate score
/// on the first 200 kB beats an error dialog for unbounded scraper/user input
/// reaching this new IPC surface).
#[test]
fn resolve_resume_and_text_clamps_oversized_job_text() {
    let (_dir, store) = scoring_store();
    store
        .insert(&DocumentRecord {
            title: "Resume".into(),
            name: "resume.pdf".into(),
            ..resume_doc("doc-1", RESUME_TEXT)
        })
        .unwrap();

    let oversized = "x".repeat(MAX_JOB_DESCRIPTION_BYTES + 500);
    let (resume, clamped) =
        resolve_resume_and_text(&store, "doc-1", oversized).expect("a real resume id must resolve");
    assert_eq!(resume.id, "doc-1");
    assert!(
        clamped.len() <= MAX_JOB_DESCRIPTION_BYTES,
        "job text over the cap must be truncated, not passed through unbounded"
    );
}

/// The sibling to the test above with a genuinely MULTIBYTE fixture. That
/// one clamps `MAX_JOB_DESCRIPTION_BYTES + 500` bytes of pure ASCII
/// (`"x".repeat(...)`), which never exercises `clamp_to_bytes`'s
/// char-boundary walk-back at all -- every byte offset in pure ASCII is
/// already a char boundary, so the interesting code path is untested.
/// An ASCII-only fixture hiding multibyte behaviour is the exact class
/// that produced this branch's own dotted-I/e-acute byte-offset
/// crash-loop incident: the code looked correct, the tests were green, and the
/// only input that mattered was never tried.
///
/// Places a 4-byte emoji EXACTLY straddling the cap (its first byte lands
/// at `MAX_JOB_DESCRIPTION_BYTES - 1`, one byte before it), so the naive
/// cutoff at the cap would split it mid-character and the walk-back MUST
/// move -- the SAME proven fixture shape `clamp_to_bytes` already has its
/// own direct unit test for (`oversized_input_is_clamped_rather_than_
/// processed_whole`, this file), applied through `resolve_resume_and_text`
/// instead: a future change that inlines the clamp, reorders it, or adds
/// a preprocessing step ahead of it inside THIS fn specifically would
/// still be caught here, not only at the lower-level primitive.
#[test]
fn resolve_resume_and_text_clamps_oversized_multibyte_job_text() {
    let (_dir, store) = scoring_store();
    store
        .insert(&DocumentRecord {
            title: "Resume".into(),
            name: "resume.pdf".into(),
            ..resume_doc("doc-1", RESUME_TEXT)
        })
        .unwrap();

    let oversized = "a".repeat(MAX_JOB_DESCRIPTION_BYTES - 1) + "\u{1F600}" + &"b".repeat(2_500);
    assert!(
        oversized.len() > MAX_JOB_DESCRIPTION_BYTES,
        "precondition: the fixture must actually exceed the cap"
    );

    let (resume, clamped) =
        resolve_resume_and_text(&store, "doc-1", oversized).expect("a real resume id must resolve");
    assert_eq!(resume.id, "doc-1");

    // Absolute against the cap, never against the input's own length --
    // proves the walk-back moved exactly one byte back from the naive
    // cutoff and stopped at the FIRST valid boundary, not some other one.
    assert_eq!(
        clamped.len(),
        MAX_JOB_DESCRIPTION_BYTES - 1,
        "clamped multibyte job text must land exactly on the char \
         boundary immediately before the straddling character"
    );
    assert!(
        !clamped.contains('\u{1F600}'),
        "the whole straddling character must be dropped, not partially \
         included"
    );
    assert!(
        String::from_utf8(clamped.into_bytes()).is_ok(),
        "clamping a multibyte string must never produce invalid UTF-8"
    );
}

// ── the posting hand-off: one lock, one scan, real facts ─────────────

/// The single cache read must hand BOTH consumers the real posting.
///
/// `match_resume` resolves the live posting once and passes the facts to the
/// hard-constraint pass, instead of that pass taking the `PostingsCache` lock and
/// re-scanning for the same id — a duplicate that would run on every Jobs-page
/// call, including the `match_scores` cache hits where the score itself costs
/// nothing. Anchored on absolute values: substituting a default here is
/// invisible to every other test in the crate.
#[test]
fn posting_facts_hand_off_carries_the_real_posting() {
    let posting = serde_json::json!({
        "id": "j1",
        "title": "Rust Engineer",
        "description": "Build things.",
        "location": "Berlin, Germany",
        "remote": true,
    });
    let (text, facts) = resolve_posting(Some(&posting));
    let text = text.expect("a posting with a title and description has scorable text");
    assert!(text.contains("Rust Engineer"), "got: {text}");
    // The constraint side gets the posting's OWN fields, not a placeholder.
    assert_eq!(facts.location.as_deref(), Some("Berlin, Germany"));
    assert!(facts.board_remote);
}

/// A cache miss yields nothing for either consumer — and that is safe because
/// `score_one` returns its job-not-found error first, which `attach` passes
/// through without ever reading these facts.
#[test]
fn posting_facts_hand_off_is_empty_when_the_posting_is_not_cached() {
    let (text, facts) = resolve_posting(None);
    assert_eq!(text, None);
    assert_eq!(facts.location, None);
    assert!(!facts.board_remote);
}
