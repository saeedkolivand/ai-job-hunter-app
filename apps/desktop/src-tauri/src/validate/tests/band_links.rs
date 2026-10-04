//! A genuine body link inside the 144pt header band must never false-block, and the link
//! helpers that decide it (`topmost_n`, `canonicalize_url`).

use super::{support::*, *};

/// CodeRabbit (security re-review, round 7): the mismatch check must NOT run
/// over the full 144pt geometric band — that band is a heuristic, and a
/// genuine, correctly-placed BODY link (a job's own company site) rendering
/// early on the page (a short header + an immediate section) is not "the
/// header's" just because it falls inside the same zone. Flagging it there
/// is exactly the false-block this check exists to avoid, not to cause.
/// Narrowed to the header's own expected link COUNT: with the profile as the
/// header's source (one expected link, its website), only the topmost band
/// link is checked — the job's own company link one line down survives
/// untouched, and the export is not blocked. (A prior round's test asserted
/// the OPPOSITE of this — that the company link DID block — which was
/// itself the false-block bug, not a real regression repro.)
///
/// The regression test both reviewers were circling this round: same
/// scenario, but with the TEXT already owning the header (its own contact
/// line present) — the exact case HIGH-2 (prior round) re-pointed the
/// mismatch check onto. Must not false-block either — same narrowing, same
/// reasoning, the profile-sourced sibling above.
///
/// Post-push security re-review (round 8): a SECOND reviewer read
/// `header_owned_links = header_links.take(allowed.len())` silently checking
/// zero links when `allowed` is empty as an unintentional hole and proposed
/// falling back to the FULL band in that case. Verified empirically (see the
/// commit history / round-8 report) before writing this: that fallback is
/// WRONG and would reintroduce the exact false-block this file's other
/// "…does_not_false_block" tests exist to prevent — a name-only header with
/// no contact profile and no email/phone-with-a-link produces `allowed = {}`
/// (a bare name is a plain `String`, never linked; a phone/location line
/// never gets a `.link` run either), so a job's own company link rendering
/// early on the page (short header, immediate EXPERIENCE section) is body
/// content, not the header's, and must not block. This is the SAME
/// invariant the other two tests pin, for the specific `allowed.is_empty()`
/// case neither of them actually exercises (both have exactly one expected
/// link) — added so the intent is an assertion, not something inferred from
/// `take(0)`'s iterator semantics.
#[test]
fn a_genuine_body_link_in_the_header_band_never_false_blocks_a_resume() {
    let resume = |contact_line: &str| {
        format!(
            "Jane Doe\n{contact_line}\nEXPERIENCE\n\
             [Acme Corp](https://acme.example.com)  2020 - Present\nSenior Engineer\n\
             - Led a team of five engineers delivering the core platform\n\n\
             SKILLS\n- Rust, TypeScript, React\n"
        )
    };
    for (text, contact, ok_why, mismatch_why) in [
        // The profile is the header's source of truth: the text has no contact line.
        (
            resume(""),
            Some(profile_with("https://example.dev/portfolio")),
            "a genuine body link rendering inside the header band must not false-block",
            "the job's own company link must never be flagged as a header mismatch",
        ),
        // A profile is supplied but irrelevant here — text already owns the
        // header, so the profile is never applied.
        (
            resume("jane@example.com\n"),
            Some(profile_with("https://example.dev/portfolio")),
            "a genuine body link rendering inside a text-owned header band must not false-block",
            "the job's own company link must never be flagged as a header mismatch",
        ),
        // No contact profile, and the text's own header (just a bare name, no
        // contact line at all) supplies no links either — `allowed` is empty.
        (
            resume(""),
            None,
            "a genuine body link inside the band of a linkless header must not false-block",
            "the job's own company link must never be flagged as a header mismatch just because the header itself has no links to compare against",
        ),
    ] {
        let report = pdf_report(&text, contact);
        assert!(report.ok, "{ok_why}: {:?}", report.issues);
        assert!(
            !report
                .issues
                .iter()
                .any(|i| i.code == "header_url_mismatch"),
            "{mismatch_why}: {:?}",
            report.issues
        );
    }
}

/// Regression for the production incident (a macOS user's PDF export blocked
/// with no way out): a cover letter's opening paragraph can legitimately
/// contain a markdown link (e.g. a company-research URL the AI echoes in its
/// first sentence). With a short letterhead — no date, no recipient block —
/// that link can land inside the same 144pt header band the letterhead's own
/// link renders in. It must never be mistaken for a header link and block the
/// export: a cover letter is never two-column, so `can_linearize` is always
/// false and a false block here has no remediation at all.
///
/// The band precondition is asserted implicitly rather than by measuring the
/// link's rect: verified by reverting the `topmost_n` narrowing in
/// `validate/header_links.rs` and re-running, which fails here with exactly
/// `header_url_mismatch` on `acme.example.com`. That failure is only reachable
/// if the body link IS inside the 144pt band, so the fixture cannot silently
/// drift into passing for the wrong reason without the narrowing also
/// becoming untested — at which point this test starts passing on `main` too.
#[test]
fn cover_letter_body_link_in_a_short_letterhead_band_does_not_false_block() {
    let request = ExportRequest {
        document_type: DocumentType::CoverLetter,
        ..pdf_request(
            "\
Jane Doe

Dear Hiring Manager,

I first learned about your team through [Acme Research](https://acme.example.com/about) \
and knew immediately I wanted to apply.

Sincerely,
Jane Doe
",
            Some(profile_with("https://example.dev/portfolio")),
        )
    };
    let (_bytes, report) = export_pdf(request);
    assert!(
        report.ok,
        "a genuine body link inside a short cover letter's header band must not false-block: {:?}",
        report.issues
    );
    assert!(
        !report
            .issues
            .iter()
            .any(|i| i.code == "header_url_mismatch"),
        "the body link must never be flagged as a header mismatch: {:?}",
        report.issues
    );
}

/// Scheme and host are lowercased and a single trailing slash goes, but only on
/// the path: the query and fragment keep their case. One row per shape.
#[test]
fn canonicalize_url_lowercases_scheme_and_host_only() {
    for (url, expected) in [
        // Query-string case (including values like `Token=ABC`) must be preserved.
        // The old code found '/' at `after.len()` but still treated `?Token=ABC` as
        // part of the authority — so `authority.to_lowercase()` clobbered the token.
        (
            "https://Example.COM/path?Token=ABC&foo=Bar",
            "https://example.com/path?Token=ABC&foo=Bar",
        ),
        // A URL with no path separator before the query must not lowercase the query.
        (
            "https://Example.COM?Token=ABC",
            "https://example.com?Token=ABC",
        ),
        // Fragment identifiers must not be lowercased or mangled.
        (
            "https://Example.COM/page#SectionTitle",
            "https://example.com/page#SectionTitle",
        ),
        // A trailing slash on the PATH (before any `?`) is stripped; the query is kept.
        (
            "https://example.com/profile/?Token=ABC",
            "https://example.com/profile?Token=ABC",
        ),
    ] {
        assert_eq!(canonicalize_url(url), expected, "{url}");
    }
}

/// Two genuinely different URLs (different hosts / paths) must never compare equal.
#[test]
fn canonicalize_url_different_urls_are_not_equal() {
    let a = canonicalize_url("https://linkedin.com/in/janedoe");
    let b = canonicalize_url("https://github.com/janedoe");
    assert_ne!(
        a, b,
        "different URLs must not collide after canonicalization"
    );
}

/// Regression: a Google Drive URL with a query containing uppercase must not be
/// lowercased — this is the exact URL shape that false-blocked exports.
#[test]
fn canonicalize_url_google_drive_link_is_stable() {
    let url = "https://drive.google.com/file/d/abc123/view?usp=drive_link";
    // Canonicalizing twice must yield the same string (idempotent).
    let once = canonicalize_url(url);
    let twice = canonicalize_url(&once);
    assert_eq!(once, twice, "canonicalize_url must be idempotent");
    // The query value must survive unchanged.
    assert!(
        once.contains("?usp=drive_link"),
        "query must survive: {once}"
    );
}

/// Regression: which band links count as "the header's own" must be decided by
/// where they sit on the page, never by `/Annots` emission order.
///
/// `page_link_annotations` returns annotations in array order, which carries no
/// guarantee of vertical position — a two-column template emits its sidebar as
/// its own run, so a body link can precede a header one. Selecting by emission
/// order would then pick the body link, raise a CRITICAL `header_url_mismatch`,
/// and make `validate_and_fix` silently re-render the document single-column.
///
/// The links below are deliberately supplied in the *worst* order: lowest on the
/// page first. Taking the first two by emission order yields the two body links;
/// taking them by geometry yields the header's.
#[test]
fn topmost_n_orders_by_geometry_not_annotation_order() {
    let link = |y: f32, url: &str| PdfLink {
        rect: [0.0, y, 100.0, y + 10.0],
        url: url.to_string(),
        page: 0,
    };
    // PDF user space is bottom-up: a larger y is higher on the page.
    let body_low = link(100.0, "https://acme.example.com/careers");
    let body_mid = link(300.0, "https://acme.example.com/team");
    let header_b = link(700.0, "https://github.com/jane");
    let header_a = link(720.0, "https://linkedin.com/in/jane");
    let emission_order = [&body_low, &body_mid, &header_b, &header_a];

    let picked = topmost_n(&emission_order, 2);

    assert_eq!(
        picked.iter().map(|l| l.url.as_str()).collect::<Vec<_>>(),
        vec!["https://linkedin.com/in/jane", "https://github.com/jane"],
        "must select the two highest links, top-down — selecting by emission \
         order would have picked the body links and false-blocked the export"
    );
}

/// `topmost_n` must not panic or over-take when asked for more links than exist.
#[test]
fn topmost_n_caps_at_the_available_link_count() {
    let only = PdfLink {
        rect: [0.0, 700.0, 100.0, 710.0],
        url: "https://linkedin.com/in/jane".to_string(),
        page: 0,
    };
    assert_eq!(topmost_n(&[&only], 5).len(), 1);
    assert!(topmost_n(&[], 3).is_empty());
}
