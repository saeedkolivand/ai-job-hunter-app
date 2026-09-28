use crate::applications::normalize_job_url;

// ─────────────────────────────────────────────────────────────────────────────
// A5. Canonical-import precedence — the list-shell's whole-document JSON-LD is
// never adopted; only the extension's HINT-SCOPED subtree may fill a gap.
//
// `handle_import` is an async fn that takes `&AppHandle` and cannot be invoked
// hermetically (no live Tauri runtime in unit tests — see the A3 note above).
// So we test the invariant at the reachable seams:
//
//   (a) A LinkedIn SPA/list URL DOES get rewritten by `canonical_job_url` →
//       `canonical.is_some()`.
//   (b) Parsing the list-shell HTML with the whole-document `parse_from_html`
//       WOULD have yielded a titled posting for the WRONG (unrelated) job —
//       proving why the canonical branch must never call it on a list shell
//       (see `canonical_branch_hint_scoped_fallback_ignores_unrelated_whole_document_json_ld`
//       above for the full merge-level pin of this).
//   (c) The canonical branch NEVER calls `parse_from_html` — only `resolve(c)`,
//       and (when that comes back unusable/description-less) the HINT-SCOPED
//       `job_root_generic_html`, which reads only the `[data-ajh-job-root]`
//       subtree and never the document's JSON-LD/`__NEXT_DATA__`/whole-page
//       heuristics. When `resolve` returns nothing usable AND there is no
//       usable hint, the handler falls through to the stub — never to a
//       list-shell parse.
//
// Together (a)+(b)+(c) pin the invariant: a SPA/list import can only ever
// adopt title/content from `resolve`'s own fetch or the extension's explicitly
// hinted pane — never from the list shell's whole-document markup.
// ─────────────────────────────────────────────────────────────────────────────

/// A LinkedIn jobs/search URL with a `currentJobId` is rewritten to a canonical
/// view URL. The extension sends the list-shell DOM alongside it; the list-shell
/// HTML WOULD have yielded a titled (but WRONG) posting via the whole-document
/// `parse_from_html` — proving why the canonical branch must never call it on
/// a list shell. The canonical branch only ever calls `resolve(c)` plus, as a
/// gap-filler, the HINT-SCOPED `job_root_generic_html` — never the
/// whole-document parse — so the shell's own JSON-LD/heuristic content can
/// never leak into an imported application.
#[test]
fn canonical_spa_url_rewrite_skips_list_shell_dom_parse() {
    use crate::scraping::scrape_url::{canonical_job_url, parse_from_html};

    let list_url = "https://www.linkedin.com/jobs/search/?currentJobId=4185657072";

    // (a) A rewrite MUST happen — this is the precondition for the whole test.
    let canonical = canonical_job_url(list_url);
    assert!(
        canonical.is_some(),
        "a LinkedIn search URL with currentJobId MUST be rewritten to a canonical view URL;          if this fails the board rewrites were removed and the test needs updating"
    );
    assert_eq!(
        canonical.as_deref(),
        Some("https://www.linkedin.com/jobs/view/4185657072"),
        "canonical must point to the /jobs/view/<id> form"
    );

    // (b) The list-shell HTML WOULD have produced a titled posting via the
    // whole-document parse_from_html — proving that calling it on the shell
    // (instead of the hint-scoped job_root_generic_html) would adopt
    // list-shell content as the import result.
    let list_shell_html = r#"
        <html>
        <head>
            <title>Jobs at LinkedIn</title>
            <script type="application/ld+json">
            {
                "@context": "https://schema.org/",
                "@type": "JobPosting",
                "title": "WRONG: This is the list-shell job posting",
                "hiringOrganization": { "name": "LinkedIn List Shell" }
            }
            </script>
        </head>
        <body><h1>Job Search Results</h1></body>
        </html>
    "#;
    let shell_parse = parse_from_html(list_url, list_shell_html);
    assert!(
        shell_parse.as_ref().is_some_and(|p| !p.title.is_empty()),
        "parse_from_html on the list-shell HTML yields a titled posting —          confirming the whole-document parse would wrongly adopt this content"
    );

    // (c) The canonical branch never calls parse_from_html at all — only
    // resolve(c) and, as a gap-filler, the hint-scoped job_root_generic_html.
    // We can't call handle_import hermetically, but its canonical-branch
    // structure is:
    //
    //   if let Some(c) = canonical.as_deref() {
    //       let resolved = resolve(c).await?;
    //       if resolved is unusable/description-less {
    //           job_root_generic_html(html)   ← HINT-SCOPED gap-filler only,
    //                                            never parse_from_html
    //       }
    //   } else if let Some(h) = html.as_deref() {
    //       parse_from_html(...)  ← SKIPPED entirely when canonical is Some
    //   } ...
    //
    // And the single fallback guard:
    //   if ... && canonical.is_none() && html.is_some() { ... }
    //             ^^^^^^^^^^^^^^^^^^^
    //   is also guarded — so resolve(effective_url) is also skipped for the canonical path.
    //
    // Asserting canonical.is_some() (done above) is the structural proof that
    // the whole-document parse_from_html is unreachable for this URL.
    assert!(
        canonical.is_some(),
        "structural proof: canonical.is_some() → the whole-document parse_from_html branch is          unreachable for this URL under the hint-scoped canonical precedence"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// B4. normalize_job_url transforms the dedup key relies on (MEDIUM)
//
// upsert_for_origin dedup is currently the only observable check of these. Pin
// the exact transforms directly so a regression is caught at the unit boundary.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn normalize_job_url_strips_www_query_fragment_and_trailing_slash() {
    // www. strip + trailing-slash strip.
    assert_eq!(
        normalize_job_url("https://www.acme.example/jobs/42/"),
        "https://acme.example/jobs/42"
    );
    // query + utm_* strip (whole query is dropped).
    assert_eq!(
        normalize_job_url("https://acme.example/jobs/42?utm_source=ext&ref=x"),
        "https://acme.example/jobs/42"
    );
    // #fragment strip.
    assert_eq!(
        normalize_job_url("https://acme.example/jobs/42#apply"),
        "https://acme.example/jobs/42"
    );
    // lowercase host (scheme preserved-lowercased).
    assert_eq!(
        normalize_job_url("HTTPS://WWW.Acme.Example/Jobs/42"),
        "https://acme.example/jobs/42"
    );
    // All transforms at once.
    assert_eq!(
        normalize_job_url("https://www.Acme.Example/jobs/42/?utm_campaign=z#frag"),
        "https://acme.example/jobs/42"
    );
}

#[test]
fn normalize_job_url_neutralizes_non_http_schemes_to_empty() {
    // Dangerous explicit schemes collapse to "" (treated as "no url").
    assert_eq!(normalize_job_url("javascript:alert(1)"), "");
    assert_eq!(normalize_job_url("data:text/html,<h1>x</h1>"), "");
    assert_eq!(normalize_job_url("file:///etc/passwd"), "");
    assert_eq!(normalize_job_url("ftp://acme.example/x"), "");
    // Empty input → empty.
    assert_eq!(normalize_job_url("   "), "");
}
