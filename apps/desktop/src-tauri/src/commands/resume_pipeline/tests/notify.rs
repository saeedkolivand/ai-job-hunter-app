use super::super::notify::run_notification;
use super::super::report;
use super::support::{fabrication_keys, report_for, CLEAN_SOURCE, FABRICATING_DRAFT};

/// **A finished run announces what it actually ended in.**
///
/// The four terminal states get four different cards, and a non-terminal status
/// gets none — a run is announced once, when it is over. `needsReview` must not
/// read as a failure (the document is usable) and `failed` must not read as
/// ready.
///
/// Asserted on the pure builder: `push_and_notify` needs an `AppHandle` this
/// crate has no harness for, so the SIDE EFFECT is one line at one call site
/// while the whole decision is here (the same seam shape as `apply_stop` and
/// `truncation_notification`).
///
/// Mutation check: return a card for `running` and the first assertion fails;
/// collapse `needsReview` onto the `completed` title and the "not a failure /
/// not clean" pair does; drop the singular arm from `review_note` and the
/// "1 flagged claim" assertion does.
#[test]
fn the_terminal_notification_reports_the_state_the_run_ended_in() {
    // Not terminal, and not a status this module knows: nothing to announce.
    assert!(run_notification("running", 3, "Senior Engineer", "Acme").is_none());
    assert!(run_notification("queued", 0, "Senior Engineer", "Acme").is_none());

    let completed = run_notification(super::super::STATUS_COMPLETED, 0, "Senior Engineer", "Acme")
        .expect("a terminal run is announced");
    assert_eq!(completed.kind, "resume.pipeline_run");
    assert_eq!(completed.body, "Senior Engineer · Acme");
    assert!(completed
        .route
        .as_ref()
        .is_some_and(|route| route.to == "/jobs"));

    let review = run_notification(
        super::super::STATUS_NEEDS_REVIEW,
        2,
        "Senior Engineer",
        "Acme",
    )
    .expect("needsReview is terminal");
    assert!(
        review.body.contains("2 flagged claims"),
        "the user is told how much is left: {}",
        review.body
    );
    assert!(
        review.title.contains("ready"),
        "needsReview is not a failure — the document exists"
    );
    assert_ne!(review.title, completed.title, "…and it is not clean either");

    // One finding is not "1 flagged claims", and ZERO is the unreviewable
    // Critical (`factual.dropped_role`), not "0 flagged claims".
    let one = run_notification(super::super::STATUS_NEEDS_REVIEW, 1, "", "").expect("terminal");
    assert!(one.body.contains("1 flagged claim needs"), "{}", one.body);
    assert!(
        one.body.starts_with("Untitled posting"),
        "a posting with no title or company still names itself: {}",
        one.body
    );
    let unreviewable =
        run_notification(super::super::STATUS_NEEDS_REVIEW, 0, "Engineer", "").unwrap();
    assert!(!unreviewable.body.contains('0'), "{}", unreviewable.body);
    assert!(unreviewable.body.contains("cannot clear"));
    assert!(unreviewable.body.starts_with("Engineer — "));

    let failed =
        run_notification(super::super::STATUS_FAILED, 0, "Senior Engineer", "Acme").unwrap();
    let cancelled =
        run_notification(super::super::STATUS_CANCELLED, 0, "Senior Engineer", "Acme").unwrap();
    let titles = [
        completed.title,
        review.title,
        failed.title,
        cancelled.title.clone(),
    ];
    let mut unique = titles.to_vec();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 4, "four outcomes, four cards: {titles:?}");
    // The cancel card exists because a cancel lands at the next stage boundary,
    // which can be minutes after the click — this is "it has stopped now".
    assert!(cancelled.body.contains("has stopped"));

    // The call SITE, grep-shaped for the same reason
    // `every_provider_calling_command_admits_before_it_spends` is: `execute`
    // needs a Tauri harness this crate does not have, so "a finished run
    // actually announces itself" is otherwise provable only by reading the code
    // — and the decision above is inert without the call.
    assert!(
        include_str!("../run.rs").contains("notify::notify_terminal("),
        "execute must push the terminal notification"
    );
}

/// **A crafted posting title cannot displace what the card is FOR.**
///
/// The label is scraped, attacker-influenceable text and it comes first in every
/// body; the store clamps the whole body to `MAX_BODY_CHARS` and this string can
/// leave the app as an OS banner. Unbudgeted, a 600-character title eats the
/// entire clamp and the user is shown pure attacker text with no clause saying
/// what happened — so the label is capped and the fixed half always survives.
///
/// Asserted against the store's OWN constant, not a second copy of 500: the
/// relation being pinned is "label + clause fits the clamp", and a test carrying
/// its own number stops testing the clamp the moment the store's changes.
///
/// Mutation check: remove the `LABEL_CAP` clamp from `posting_label` and every
/// `survives` assertion below fails.
#[test]
fn a_crafted_posting_title_cannot_displace_the_notification_clause() {
    const CLAMP: usize = crate::notifications::MAX_BODY_CHARS;
    let hostile = "T".repeat(600);
    let cases = [
        (
            super::super::STATUS_NEEDS_REVIEW,
            3usize,
            "Keep or Remove decision",
        ),
        (super::super::STATUS_NEEDS_REVIEW, 0, "cannot clear"),
        (
            super::super::STATUS_FAILED,
            0,
            "before it produced a résumé",
        ),
        (super::super::STATUS_CANCELLED, 0, "has stopped"),
    ];
    for (status, unresolved, clause) in cases {
        let body = run_notification(status, unresolved, &hostile, &hostile)
            .expect("a terminal status is announced")
            .body;
        assert!(
            body.chars().count() <= CLAMP,
            "{status}: the body must fit the store's clamp before it is cut ({} chars)",
            body.chars().count()
        );
        // The clamp the store will actually apply, applied here: the clause has
        // to survive it, not merely exist somewhere past it.
        let survives: String = body.chars().take(CLAMP).collect();
        assert!(
            survives.contains(clause),
            "{status}: {clause:?} was displaced by the scraped title — {survives}"
        );
        assert!(
            survives.starts_with("TTT"),
            "{status}: the posting still names itself"
        );
    }

    // An ordinary title is untouched — the cap is a backstop, not a formatter.
    let ordinary = run_notification(super::super::STATUS_COMPLETED, 0, "Senior Engineer", "Acme")
        .expect("terminal")
        .body;
    assert_eq!(ordinary, "Senior Engineer · Acme");
}

/// **SEC-LOW-2.** An embedded newline (or other whitespace run) in the
/// scraped title/company — or PR-3's text-path `jobTitle`/`companyName`,
/// neither line-shape checked upstream — must not survive into a two-line
/// OS notification body. Mutation check: revert `posting_label` to
/// `title.trim()`/`company.trim()` (drop `collapse_whitespace`) and this
/// fails.
#[test]
fn an_embedded_newline_in_the_posting_title_does_not_split_the_notification_body() {
    let body = run_notification(
        super::super::STATUS_COMPLETED,
        0,
        "Staff\nEngineer",
        "Acme\tCorp",
    )
    .expect("terminal")
    .body;
    assert_eq!(body, "Staff Engineer · Acme Corp");
    assert_eq!(body.lines().count(), 1, "the body must stay one line");
}

/// A crafted `<b>`/`<a href>` in the scraped title/company — or PR-3's
/// text-path `jobTitle`/`companyName` — must not survive as markup: a Linux
/// notification daemon that advertises libnotify body-markup would render it
/// as a tag rather than text. Mutation check: drop the `escape_markup` call
/// from `posting_label` and this fails.
#[test]
fn a_crafted_html_tag_in_the_posting_title_is_escaped_not_rendered() {
    let body = run_notification(
        super::super::STATUS_COMPLETED,
        0,
        "<b>Staff Engineer</b>",
        "Acme & Co <script>",
    )
    .expect("terminal")
    .body;
    assert_eq!(
        body,
        "&lt;b&gt;Staff Engineer&lt;/b&gt; · Acme &amp; Co &lt;script&gt;"
    );
    assert!(!body.contains('<'));
    assert!(!body.contains('>'));
}

/// The number in the notification is the number the review panel will show — one
/// definition of "undecided", read from the persisted wrapper.
///
/// Mutation check: count every fabrication instead of the undecided ones and the
/// post-verdict assertion fails; make `unresolved_count` panic-free-but-wrong on
/// a bad blob (return 1) and the last assertion does.
#[test]
fn the_review_notification_counts_the_findings_the_panel_lists() {
    let report = report_for(FABRICATING_DRAFT, CLEAN_SOURCE);
    let wrapper = report::build("quality", 1, Some((&report, FABRICATING_DRAFT)), None);
    let keys = fabrication_keys(&wrapper);
    assert_eq!(
        report::unresolved_count(&wrapper, FABRICATING_DRAFT, ""),
        keys.len()
    );
    assert!(report::has_unresolved(&wrapper, FABRICATING_DRAFT, ""));

    let body = run_notification(
        super::super::STATUS_NEEDS_REVIEW,
        report::unresolved_count(&wrapper, FABRICATING_DRAFT, ""),
        "Engineer",
        "Acme",
    )
    .expect("terminal")
    .body;
    assert!(body.contains(&keys.len().to_string()), "{body}");

    let mut current = wrapper;
    for key in &keys {
        current = report::record_decision(&current, key, "keep").expect("a known key");
    }
    assert_eq!(report::unresolved_count(&current, FABRICATING_DRAFT, ""), 0);
    assert!(!report::has_unresolved(&current, FABRICATING_DRAFT, ""));

    // No report at all (a fast-path row, or a run that failed before validate)
    // counts nothing rather than inventing review work.
    assert_eq!(report::unresolved_count("", "", ""), 0);
    assert_eq!(report::unresolved_count("not json", "", ""), 0);
    assert_eq!(report::unresolved_count("{}", "", ""), 0);
}
