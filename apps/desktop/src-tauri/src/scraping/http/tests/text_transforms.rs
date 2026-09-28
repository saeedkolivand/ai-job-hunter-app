//! Pure (no-network) tests: `FetchOptions` defaults, `safe_log_url`
//! redaction, and the `strip_html`/`html_to_text`/`html_to_markdown`
//! transforms.

use super::super::*;

#[test]
fn test_fetch_options_default() {
    let opts = FetchOptions::default();
    assert!(opts.headers.is_none());
    assert!(opts.method.is_none());
    assert!(opts.body.is_none());
    // Default retries raised to 2 to allow one 429/503 backoff before giving up.
    assert_eq!(opts.retries, 2);
    // `None` keeps the shared browser-shaped DEFAULT_UA — a board opts INTO an
    // identifying UA (e.g. freehire), it is never on by default.
    assert!(opts.user_agent.is_none());
}

/// `FetchOptions.timeout` backward-safe contract (item 4):
/// - `Default` leaves `timeout: None` (backward-safe — no global ceiling for existing callers).
/// - An explicit `Some(Duration)` round-trips correctly (opt-in ceiling is preserved).
///
/// No network call needed — this is a pure struct field assertion.
#[test]
fn test_fetch_options_timeout_field_contract() {
    // Default must leave timeout as None — every existing call-site relies on
    // the no-global-timeout contract (`..Default::default()`).
    let defaults = FetchOptions::default();
    assert!(
        defaults.timeout.is_none(),
        "Default FetchOptions must have timeout: None (backward-safe)"
    );

    // Opt-in: an explicit Some(Duration) survives round-trip through the struct.
    let with_timeout = FetchOptions {
        timeout: Some(Duration::from_millis(1)),
        ..Default::default()
    };
    assert_eq!(
        with_timeout.timeout,
        Some(Duration::from_millis(1)),
        "FetchOptions.timeout should round-trip as Some(1ms)"
    );
    // Other fields stay at their defaults when only timeout is set.
    assert!(with_timeout.headers.is_none());
    assert_eq!(with_timeout.retries, 2);
}

/// `redact_path: false` (the default) keeps the path — the shared query-only
/// redaction is what protects the common case (Adzuna/Comeet-style secrets in
/// the query string).
#[test]
fn test_safe_log_url_keeps_path_by_default() {
    let url = "https://jooble.org/api/super-secret-key";
    assert_eq!(
        safe_log_url(url, false),
        "https://jooble.org/api/super-secret-key"
    );
}

/// `redact_path: true` — for endpoints that embed a secret directly in the URL
/// PATH (e.g. Jooble's `POST /api/{apiKey}`) — must drop the path entirely so
/// the credential never reaches a non-2xx / schema-drift log line.
#[test]
fn test_safe_log_url_redacts_path_when_requested() {
    let url = "https://jooble.org/api/super-secret-key";
    let redacted = safe_log_url(url, true);
    assert_eq!(redacted, "https://jooble.org");
    assert!(!redacted.contains("super-secret-key"));
}

/// Regression guard: the query string (Adzuna `app_key`, Comeet token, …) must
/// never appear in the log-safe URL regardless of `redact_path`.
#[test]
fn test_safe_log_url_never_includes_query() {
    let url = "https://api.adzuna.com/v1/api/jobs/de/search/1?app_key=secret123";
    assert!(!safe_log_url(url, false).contains("secret123"));
    assert!(!safe_log_url(url, true).contains("secret123"));
}

#[test]
fn test_strip_html_basic() {
    let html = "<p>Hello <b>World</b></p>";
    let result = strip_html(html);
    assert_eq!(result, "Hello World");
}

#[test]
fn test_strip_html_script() {
    let html = "<script>alert('xss')</script><p>Content</p>";
    let result = strip_html(html);
    assert_eq!(result, "Content");
}

#[test]
fn test_strip_html_style() {
    let html = "<style>body { color: red; }</style><p>Content</p>";
    let result = strip_html(html);
    assert_eq!(result, "Content");
}

#[test]
fn test_strip_html_entities() {
    let html = "Hello &amp; World &lt;3";
    let result = strip_html(html);
    assert_eq!(result, "Hello & World <3");
}

#[test]
fn test_strip_html_nbsp() {
    let html = "Hello&nbsp;&nbsp;World";
    let result = strip_html(html);
    assert_eq!(result, "Hello World");
}

#[test]
fn test_strip_html_quotes() {
    let html = "&quot;Test&quot; and &#39;quote&#39;";
    let result = strip_html(html);
    assert_eq!(result, "\"Test\" and 'quote'");
}

#[test]
fn test_strip_html_whitespace() {
    let html = "<p>Hello</p>   <p>World</p>";
    let result = strip_html(html);
    assert_eq!(result, "Hello World");
}

#[test]
fn test_strip_html_empty() {
    let html = "";
    let result = strip_html(html);
    assert_eq!(result, "");
}

#[test]
fn test_strip_html_nested_tags() {
    let html = "<div><span><b>Bold</b></span></div>";
    let result = strip_html(html);
    assert_eq!(result, "Bold");
}

#[test]
fn test_html_to_text_preserves_structure() {
    let html = "<p>Intro paragraph.</p><p>Responsibilities:</p><ul><li>Build features</li><li>Write tests</li></ul>";
    let result = html_to_text(html);
    assert_eq!(
        result,
        "Intro paragraph.\nResponsibilities:\n• Build features\n• Write tests"
    );
}

#[test]
fn test_html_to_text_br_and_entities() {
    let html = "Line one<br>Line two &amp; more";
    let result = html_to_text(html);
    assert_eq!(result, "Line one\nLine two & more");
}

#[test]
fn test_html_to_text_caps_blank_lines() {
    let html = "<div>A</div><div></div><div></div><div>B</div>";
    let result = html_to_text(html);
    assert_eq!(result, "A\n\nB");
}

// ── html_to_markdown ────────────────────────────────────────────────────────

/// Representative job HTML with heading, paragraph, list, and bold renders to
/// proper Markdown (heading prefix, list dashes/asterisks, **bold**).
#[test]
fn test_html_to_markdown_job_html() {
    let html = r#"
        <h3>About the Role</h3>
        <p>We are looking for a <strong>Senior Engineer</strong> to join our team.</p>
        <ul>
            <li>Build scalable systems</li>
            <li>Write clean code</li>
        </ul>
    "#;
    let md = html_to_markdown(html);
    // Heading renders as ATX-style Markdown heading.
    assert!(
        md.contains("### About the Role"),
        "expected h3 → '### About the Role', got: {md}"
    );
    // Bold renders as **…**.
    assert!(
        md.contains("**Senior Engineer**"),
        "expected **Senior Engineer**, got: {md}"
    );
    // List items render as Markdown bullets.
    assert!(
        md.contains("Build scalable systems"),
        "expected list item text, got: {md}"
    );
    assert!(
        md.contains("Write clean code"),
        "expected list item text, got: {md}"
    );
    // Must not be a flat single-line blob — structure is preserved.
    assert!(md.contains('\n'), "expected multi-line output, got: {md}");
}

/// Empty HTML must return an empty string (no panic, no extraneous whitespace).
#[test]
fn test_html_to_markdown_empty() {
    assert_eq!(html_to_markdown(""), "");
}

/// Plain text (no HTML tags) passes through unchanged and does not panic.
#[test]
fn test_html_to_markdown_plain_text() {
    let plain = "Senior Rust Engineer — remote, full-time";
    let result = html_to_markdown(plain);
    assert!(
        result.contains("Senior Rust Engineer"),
        "plain text must pass through, got: {result}"
    );
}

/// BUG-VERIFY (Step 1): plain-text input containing literal `**` markers must NOT
/// be escaped to `\*\*` by htmd.  Before the fix, htmd treated the plain text as
/// an HTML document, found no tags, but still escaped markdown special chars in
/// every text node — producing `\*\*We help…\*\*` which react-markdown renders
/// as literal asterisks instead of bold.
///
/// After the fix the plain-text path bypasses htmd entirely and returns the input
/// unchanged, so the full output must equal the input exactly.
#[test]
fn test_html_to_markdown_plain_text_bold_markers_not_escaped() {
    let plain = "**We help the world run better** At SAP, we keep it simple.\n\n**Summary & Role Information:**\nYou will lead the team.";
    let result = html_to_markdown(plain);
    // Exact-equality: the plain-text path must be a no-op (trimmed input returned as-is).
    assert_eq!(
        result, plain,
        "plain-text ** must pass through unescaped and unchanged"
    );
}

/// After the fix, HTML <strong> still produces clean ** (regression guard).
#[test]
fn test_html_to_markdown_strong_tag_still_produces_bold() {
    let html = "<p><strong>We help the world run better</strong> At SAP.</p>";
    let result = html_to_markdown(html);
    assert!(
        result.contains("**We help the world run better**"),
        "HTML <strong> must still produce ** bold, got: {result}"
    );
    assert!(
        !result.contains("\\*\\*"),
        "no escaped \\*\\* in HTML path, got: {result}"
    );
}

/// Plain-text input with double-newline paragraph breaks must preserve them
/// (react-markdown needs a blank line to render separate paragraphs).
/// Exact-equality: the plain-text path returns the input unchanged.
#[test]
fn test_html_to_markdown_plain_text_paragraph_breaks_preserved() {
    let plain = "First paragraph.\n\nSecond paragraph.";
    assert_eq!(
        html_to_markdown(plain),
        plain,
        "plain-text double newlines must be preserved as-is"
    );
}

/// When htmd produces an empty result (e.g. HTML that is only whitespace/comments)
/// the fallback to html_to_text fires and does not panic.
#[test]
fn test_html_to_markdown_falls_back_on_empty_output() {
    // An HTML comment — htmd strips it and returns ""; we fall back to html_to_text
    // which also returns "". The important thing is no panic and no empty-check bypass.
    let html = "<!-- just a comment -->";
    let result = html_to_markdown(html);
    // Both paths return empty for a comment-only input; assert it doesn't panic and
    // returns a clean string (trimmed).
    assert_eq!(result.trim(), result, "result must already be trimmed");
}
