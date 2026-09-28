//! The extension's `[data-ajh-job-root]` hint: per-field merge precedence
//! (hint vs. meta description vs. JSON-LD), the thin-hint/hostile-hint
//! fallback paths, and the script/style-stripping regexes it relies on.

use super::super::generic::parse_generic_html;
use super::super::html_fallback::JOB_ROOT_SCRIPT_STYLE_RE;
use super::super::*;

// ── `[data-ajh-job-root]` hint (PR 3: desktop parser consumes the hint) ──────
//
// The extension's Scan-mode capture (`markLikelyJobNode` in
// apps/extension/src/content.ts) best-effort marks one node with
// `data-ajh-job-root="true"` before handing the full outerHTML to the desktop.
// These tests cover the generic-fallback preference for that hinted subtree,
// added ONLY to `job_root_generic_html` (and `parse_from_html`'s per-field
// merge of its output) — the JSON-LD and __NEXT_DATA__ paths above are
// untouched and still win when present.

#[test]
fn test_parse_from_html_job_root_hint_wins_over_larger_block() {
    // No <title> tag, so parse_generic_html's `title, h1` selector would fall to
    // the FIRST <h1> in the document — which, without the hint, is the unrelated
    // sidebar block's heading (DOM-order first). Likewise main_content_text picks
    // the LARGEST of the two <article> blocks, and the padded sidebar block is
    // deliberately longer than the real posting. Both whole-document heuristics
    // are wrong on this page; the `[data-ajh-job-root]` hint on the real posting
    // must correct both title and description — the per-field merge overrides
    // each field independently, and here BOTH fields happen to be present in
    // the hinted subtree (see the thin-body test below for the single-field
    // case, where only one of the two is overridden).
    let padded = "Related jobs you might like, sponsored content, more links. ".repeat(20);
    let html = format!(
        r#"<html><head></head><body>
            <nav>
                <article><h1>Related: Other Job For SEO</h1><p>{padded}</p></article>
            </nav>
            <article data-ajh-job-root="true">
                <h1>Backend Engineer</h1>
                <p>We are looking for a backend engineer to build resilient distributed systems.</p>
            </article>
        </body></html>"#
    );
    let posting = parse_from_html("https://acme.example/j/hint-1", &html)
        .expect("a hinted subtree with real content must yield a posting");
    assert_eq!(
        posting.title, "Backend Engineer",
        "hinted subtree's h1 must win over the first (unrelated) h1 in DOM order"
    );
    let desc = posting.description.as_deref().unwrap_or_default();
    assert!(
        desc.contains("resilient distributed systems"),
        "description must come from the hinted subtree, got: {desc}"
    );
    assert!(
        !desc.contains("Related jobs"),
        "description must NOT be the larger unrelated sidebar block, got: {desc}"
    );
}

#[test]
fn test_parse_from_html_job_root_hint_unusable_falls_through() {
    // The hinted node is empty (whitespace only, no h1, no text) — a mis-marked
    // hint. job_root_generic_html() yields ("", None) for it, so neither field
    // of the per-field merge overrides, and parse_from_html falls through to
    // parse_generic_html's <title>/meta-description path, exactly as if no hint
    // existed at all.
    let html = r#"
        <html>
            <head>
                <title>Backend Engineer</title>
                <meta name="description" content="Build APIs at Acme">
            </head>
            <body>
                <div data-ajh-job-root="true">   </div>
            </body>
        </html>
    "#;
    let posting = parse_from_html("https://acme.example/j/hint-2", html)
        .expect("an unusable hint must still fall through to a usable posting");
    assert_eq!(
        posting.title, "Backend Engineer",
        "empty hinted node must fall through to the <title> tag"
    );
    assert_eq!(
        posting.description.as_deref(),
        Some("Build APIs at Acme"),
        "empty hinted node must fall through to the meta description"
    );
}

#[test]
fn test_parse_from_html_no_hint_is_byte_identical_to_generic_path() {
    // No `[data-ajh-job-root]` anywhere in this document — job_root_generic_html
    // must return None, so parse_from_html falls through to parse_generic_html
    // exactly as it did before this hint feature existed. Assert byte-identical
    // equality against calling parse_generic_html directly (the no-hint floor),
    // not just "looks right" — this is the guarantee that the server-fetch
    // resolve path (which never has a hint) is provably unchanged.
    let html = r#"
        <html>
            <head>
                <title>Backend Engineer</title>
                <meta name="description" content="Build APIs">
                <meta property="og:site_name" content="Acme Corp">
            </head>
            <body></body>
        </html>
    "#;
    let (expected_title, expected_description) = parse_generic_html(html);
    let posting = parse_from_html("https://acme.example.com/jobs/9", html)
        .expect("a title is present, so a posting is built");
    assert_eq!(posting.title, expected_title);
    assert_eq!(posting.description, expected_description);
    assert_eq!(posting.company, "Acme Corp");
}

#[test]
fn test_parse_from_html_hostile_hint_falls_through_safely() {
    // The hinted node contains ONLY a script tag and whitespace — a hostile/
    // mis-marked hint. html_to_markdown strips script content, so the hint
    // yields an empty description and no title; neither field of the per-field
    // merge overrides, and parse_from_html falls through to the whole-document
    // heuristic chain (here: the <main> block) — never producing a worse
    // (empty) result than the no-hint path would.
    let html = r#"
        <html>
            <head><title>Backend Engineer</title></head>
            <body>
                <div data-ajh-job-root="true">
                    <script>trackImpression();</script>


                </div>
                <main><p>We are hiring a backend engineer to build resilient distributed systems and own the platform end to end.</p></main>
            </body>
        </html>
    "#;
    let posting = parse_from_html("https://acme.example/j/hint-4", html)
        .expect("a hostile hint must still fall through to a usable posting");
    assert_eq!(posting.title, "Backend Engineer");
    let desc = posting.description.as_deref().unwrap_or_default();
    assert!(
        desc.contains("resilient distributed systems"),
        "description must come from the <main> fallback, not the hostile hint, got: {desc}"
    );
}

#[test]
fn test_parse_from_html_job_root_hint_thin_body_merges_per_field() {
    // The hinted node has ONLY a title — no body text (the real description
    // lives elsewhere, e.g. rendered client-side inside an ATS iframe the
    // outerHTML capture can't see). A wholesale hint substitution would have
    // discarded the document's real meta description in favor of a
    // title-redundant stub ("Backend Engineer" markdownified); the per-field
    // merge must take the better title from the hint while leaving the real
    // meta description untouched.
    let html = r#"
        <html>
            <head>
                <title>Careers at Acme</title>
                <meta name="description" content="We are hiring a backend engineer to build resilient distributed systems.">
            </head>
            <body>
                <main data-ajh-job-root="true"><h1>Backend Engineer</h1></main>
            </body>
        </html>
    "#;
    let posting = parse_from_html("https://acme.example/j/hint-thin", html)
        .expect("a title is present, so a posting is built");
    assert_eq!(
        posting.title, "Backend Engineer",
        "title must come from the hint, not the generic <title> tag"
    );
    assert_eq!(
        posting.description.as_deref(),
        Some("We are hiring a backend engineer to build resilient distributed systems."),
        "description must stay the real meta description, not a title-redundant hint stub"
    );
}

#[test]
fn test_job_root_script_style_re_strips_case_variant_and_nested_tags() {
    // Uppercase/mixed-case tags (real-world markup isn't always lowercase) and a
    // <script> whose own body contains markup-shaped text (must not confuse the
    // non-greedy match into stopping early or matching across tags).
    let html = concat!(
        "<P>Keep me</P>",
        "<SCRIPT type=\"text/javascript\">if (x < 1) { document.write(\"<b>fake</b>\"); }</SCRIPT>",
        "<Style>.x { color: red; }</Style>",
        "<p>Also keep me</p>",
    );
    let cleaned = JOB_ROOT_SCRIPT_STYLE_RE.replace_all(html, " ");
    assert!(
        !cleaned.contains("document.write"),
        "script content must be stripped: {cleaned}"
    );
    assert!(
        !cleaned.contains("color: red"),
        "style content must be stripped: {cleaned}"
    );
    assert!(cleaned.contains("Keep me"));
    assert!(cleaned.contains("Also keep me"));
}

#[test]
fn test_parse_from_html_thin_hint_last_resort_does_not_scan_whole_document() {
    // Regression for a HIGH ensemble-review finding: a thin hint (title-only,
    // no body) on a page with no <meta name="description"> anywhere left
    // `description` `None` after the per-field merge, so the OLD code ran
    // `main_content_text` over the WHOLE document as a last resort — which
    // can land on an unrelated decoy block bigger than the actual posting.
    // Once the hint supplied a real title, that whole-document last resort
    // must not run at all; description stays `None` rather than risk the
    // decoy text.
    let padded = "Totally unrelated marketing copy about our great company culture. ".repeat(20);
    let html = format!(
        r#"<html><body>
            <article><p>{padded}</p></article>
            <div data-ajh-job-root="true"><h1>Backend Engineer</h1></div>
        </body></html>"#
    );
    let posting = parse_from_html("https://acme.example/j/thin-hint-no-scan", &html)
        .expect("a hinted title alone still yields a posting");
    assert_eq!(posting.title, "Backend Engineer");
    assert_eq!(
        posting.description, None,
        "a thin hint's last resort must stay None, not the unrelated decoy article, got: {:?}",
        posting.description
    );
}

#[test]
fn test_job_root_generic_html_keeps_non_title_h1_headings() {
    // JOB_ROOT_TITLE_RE previously stripped EVERY <h1> in the hinted subtree,
    // not just the title's — a job page that styles section headings (e.g.
    // "Responsibilities") as <h1> would silently lose them from the
    // description. Only the FIRST <h1> (the title) is excluded now.
    let html = r#"
        <html><body>
            <div data-ajh-job-root="true">
                <h1>Backend Engineer</h1>
                <p>We build distributed systems.</p>
                <h1>Responsibilities</h1>
                <ul><li>Design APIs</li></ul>
            </div>
        </body></html>
    "#;
    let (title, description) = job_root_generic_html(html).expect("hint node present");
    assert_eq!(title, "Backend Engineer");
    let desc = description.unwrap_or_default();
    assert!(
        desc.contains("Responsibilities"),
        "a non-title h1 section heading must survive in the description, got: {desc}"
    );
}

#[test]
fn test_parse_from_html_job_root_hint_description_only_leaves_title_alone() {
    // Mirror of the thin-body test above: the hinted node has body text but no
    // <h1>. job_root_generic_html() yields ("", Some(desc)), so the per-field
    // merge must take the hint's description while leaving the document's own
    // <title> untouched.
    let html = r#"
        <html>
            <head><title>Careers at Acme</title></head>
            <body>
                <div data-ajh-job-root="true"><p>We are hiring a backend engineer to build resilient distributed systems.</p></div>
            </body>
        </html>
    "#;
    let posting = parse_from_html("https://acme.example/j/hint-desc-only", html)
        .expect("a title is present, so a posting is built");
    assert_eq!(
        posting.title, "Careers at Acme",
        "no h1 in the hinted node → title must stay the document's own <title>"
    );
    let desc = posting.description.as_deref().unwrap_or_default();
    assert!(
        desc.contains("resilient distributed systems"),
        "hint's body text must override the (absent) base description, got: {desc}"
    );
}

#[test]
fn test_parse_from_html_hint_description_beats_meta_description() {
    // Precedence pin: when the hinted subtree has its own body text, it wins
    // over a real document-level <meta name="description"> — the per-field
    // merge overrides `description` whenever the hint found ANY text, not
    // only when the base pass came back empty.
    let html = r#"
        <html>
            <head>
                <title>Careers at Acme</title>
                <meta name="description" content="Acme is a great place to work, join our team today.">
            </head>
            <body>
                <div data-ajh-job-root="true">
                    <h1>Backend Engineer</h1>
                    <p>We are hiring a backend engineer to own resilient distributed systems end to end.</p>
                </div>
            </body>
        </html>
    "#;
    let posting = parse_from_html("https://acme.example/j/hint-vs-meta", html)
        .expect("a title is present, so a posting is built");
    let desc = posting.description.as_deref().unwrap_or_default();
    assert!(
        desc.contains("own resilient distributed systems"),
        "hint body text must win over the real meta description, got: {desc}"
    );
    assert!(
        !desc.contains("great place to work"),
        "the meta description must be overridden, not merged, got: {desc}"
    );
}

#[test]
fn test_parse_from_html_json_ld_beats_hint() {
    // Precedence pin: JSON-LD JobPosting is applied AFTER the hint merge in
    // parse_from_html and unconditionally overrides both fields when present —
    // structured data always wins over the DOM hint, even when the hint itself
    // found usable text.
    let html = r#"
        <html>
            <head>
                <title>Careers at Acme</title>
                <script type="application/ld+json">
                {
                    "@type": "JobPosting",
                    "title": "Senior Backend Engineer",
                    "description": "The structured JSON-LD description wins."
                }
                </script>
            </head>
            <body>
                <div data-ajh-job-root="true">
                    <h1>Backend Engineer</h1>
                    <p>The hinted DOM description must lose to JSON-LD.</p>
                </div>
            </body>
        </html>
    "#;
    let posting = parse_from_html("https://acme.example/j/hint-vs-jsonld", html)
        .expect("a title is present, so a posting is built");
    assert_eq!(posting.title, "Senior Backend Engineer");
    assert_eq!(
        posting.description.as_deref(),
        Some("The structured JSON-LD description wins.")
    );
}

#[test]
fn test_parse_from_html_linkedin_search_shell_pane_wins_over_list_shell() {
    // Shaped like a LinkedIn search/collections view (`?currentJobId=…`): a
    // list-shell pane with several unrelated job cards, and a detail pane for
    // the SELECTED job carrying `data-ajh-job-root="true"` (per content.ts's
    // pane-first `JOB_NODE_CANDIDATES` — see extension_bridge::import_flow's
    // canonical-branch DOM fallback that consumes this same hint). The
    // description must come from the hinted detail pane, not the list cards.
    let html = r#"<html><head><title>LinkedIn Job Search | LinkedIn</title></head><body>
        <main>
            <ul class="jobs-search__results-list">
                <li><a>Unrelated Job Card One</a><p>Some other company, some other role.</p></li>
                <li><a>Unrelated Job Card Two</a><p>Another company, another role entirely.</p></li>
            </ul>
            <div class="jobs-details" data-ajh-job-root="true">
                <h1>Staff Backend Engineer</h1>
                <div class="jobs-description__content">
                    <p>We are hiring a Staff Backend Engineer to own resilient
                    distributed systems and the API platform end to end.</p>
                </div>
            </div>
        </main>
    </body></html>"#;
    let posting = parse_from_html("https://www.linkedin.com/jobs/view/4185657072", html)
        .expect("a hinted detail pane must yield a posting");
    assert_eq!(
        posting.title, "Staff Backend Engineer",
        "title must come from the hinted detail pane, not an unrelated list card"
    );
    let desc = posting.description.as_deref().unwrap_or_default();
    assert!(
        desc.contains("resilient distributed systems"),
        "description must come from the hinted detail pane, got: {desc}"
    );
    assert!(
        !desc.contains("Unrelated Job Card"),
        "description must NOT leak the list-shell's other job cards, got: {desc}"
    );
}
