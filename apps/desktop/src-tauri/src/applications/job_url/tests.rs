use super::*;

#[test]
fn normalize_strips_www_query_fragment_and_trailing_slash() {
    assert_eq!(
        normalize_job_url("https://WWW.Example.com/Jobs/123/?utm=x#frag"),
        "https://example.com/jobs/123"
    );
    assert_eq!(
        normalize_job_url("https://example.com/"),
        "https://example.com"
    );
    assert_eq!(normalize_job_url("  "), "");
    assert_eq!(
        normalize_job_url("https://www.acme.io/job/9/"),
        normalize_job_url("https://acme.io/job/9?ref=foo")
    );
}

#[test]
fn rejects_dangerous_url_schemes_to_empty() {
    // Explicit non-http(s) schemes are neutralized to "" (treated as "no url")
    // so an import-borne or Track-modal payload is never stored as an openable link.
    // `javascript:` has a scheme but no `://` — the `scheme:` form must be caught.
    assert_eq!(normalize_job_url("javascript:alert(1)"), "");
    assert_eq!(
        normalize_job_url("data:text/html,<script>alert(1)</script>"),
        ""
    );
    assert_eq!(normalize_job_url("file:///etc/passwd"), "");
    assert_eq!(normalize_job_url("vbscript:msgbox(1)"), "");
    assert_eq!(normalize_job_url("blob:https://evil.example/uuid"), "");
    // Case-insensitive scheme detection: mixed-case dangerous scheme still rejected.
    assert_eq!(normalize_job_url("JavaScript:alert(1)"), "");
}

/// [`decode_unreserved`] sits BESIDE the normalizer and must never be able to
/// restructure a url: decoding a RESERVED escape (a `%2F` becoming a path
/// separator, a `%3A` a scheme colon) is the one thing RFC 3986 §6.2.2.2
/// excludes from syntax-based normalization, and the reason `urlencoding::decode`
/// is unusable here. Mutation-checked during authoring by decoding `%2F` and
/// watching this fail.
#[test]
fn decode_unreserved_never_decodes_a_reserved_escape() {
    let reserved = "https://x.example%2Fcom/a%3Ab?q=1%3F2%233&p=100%25";
    assert_eq!(
        decode_unreserved(reserved),
        reserved,
        "every reserved escape must survive byte-for-byte"
    );
    // …while an unreserved one in the same string still decodes.
    assert_eq!(decode_unreserved("%41%2D%5F%7E%2E"), "A-_~.");
    assert_eq!(decode_unreserved("%41%2F%7E"), "A%2F~");
}

/// A `%` that isn't a complete escape is data, not a parse error — it is copied
/// through, never consumed, and never panics on a truncated tail.
#[test]
fn decode_unreserved_leaves_a_malformed_escape_untouched() {
    for input in ["%", "%2", "%ZZ", "100%", "%%41", "a%g1b"] {
        let expected = if input == "%%41" { "%A" } else { input };
        assert_eq!(
            decode_unreserved(input),
            expected,
            "malformed escape mishandled: {input}"
        );
    }
}

/// The decode is a READER's leniency: a url that goes through the normalizer is
/// stored exactly as it is today, percent-escapes intact, so the persisted dedup
/// key and its byte-identical TS mirror keep their current meaning.
#[test]
fn normalize_job_url_still_does_no_percent_decoding() {
    assert_eq!(
        normalize_job_url("https://example.com/jobs/a%2Db"),
        "https://example.com/jobs/a%2db"
    );
}

#[test]
fn embedded_control_characters_cannot_smuggle_a_scheme_past_the_guard() {
    // HTML and the WHATWG URL parser REMOVE embedded tab/CR/LF before parsing, so
    // this string is `javascript:` to any consumer — while a raw-byte scheme scan
    // sees the scheme-less `"java"` and would store the payload verbatim.
    // Stripping C0 controls first makes the guard see what a consumer sees.
    assert_eq!(normalize_job_url("java\nscript:alert(1)"), "");
    assert_eq!(normalize_job_url("java\tscript:alert(1)"), "");
    assert_eq!(normalize_job_url("java\rscript:alert(1)"), "");
    assert_eq!(normalize_job_url("ja\u{0}vascript:alert(1)"), "");
    assert_eq!(normalize_job_url("da\u{7F}ta:text/html,x"), "");

    // A control character anywhere else is removed too, so nothing unprintable is
    // ever stored as part of the dedup key.
    assert_eq!(
        normalize_job_url("https://example.com/job/\u{0}1"),
        "https://example.com/job/1"
    );
    // Leading whitespace exposed by the removal is still trimmed away.
    assert_eq!(
        normalize_job_url("\u{0} https://example.com/job/1"),
        "https://example.com/job/1"
    );
    // A control-only input degrades to "no url" rather than a bare host.
    assert_eq!(normalize_job_url("\u{0}\u{1}"), "");
}

#[test]
fn allows_http_and_https_including_mixed_case_scheme() {
    // http(s) round-trips with the exact prior normalization; mixed-case scheme is
    // lowercased like before and is NOT rejected by the dangerous-scheme guard.
    assert_eq!(
        normalize_job_url("HTTP://Example.com/Job/1/"),
        "http://example.com/job/1"
    );
    assert_eq!(
        normalize_job_url("HTTPS://WWW.Acme.io/job/9?ref=foo"),
        "https://acme.io/job/9"
    );
}

#[test]
fn scheme_less_input_with_colon_in_path_is_not_misclassified() {
    // A `:` inside the path/query must NOT look like a scheme — scheme-less input
    // keeps its exact prior behavior (host/path preserved, query dropped).
    assert_eq!(
        normalize_job_url("example.com/job/9?x=a:b"),
        "example.com/job/9"
    );
    assert_eq!(
        normalize_job_url("www.example.com/jobs/123/"),
        "example.com/jobs/123"
    );
}

#[test]
fn retains_per_host_identifying_query_params_for_dedup() {
    // Indeed carries the job id in the query (`?jk=<id>`). Two DISTINCT ids must
    // yield two DISTINCT keys — the collision bug was that every Indeed job
    // normalized to a single `/viewjob` key and merged onto one Application.
    assert_ne!(
        normalize_job_url("https://indeed.com/viewjob?jk=aaa"),
        normalize_job_url("https://indeed.com/viewjob?jk=bbb")
    );
    // The identifying param survives verbatim (lowercased with the rest of the url).
    assert_eq!(
        normalize_job_url("https://www.indeed.com/viewjob?jk=abc123"),
        "https://indeed.com/viewjob?jk=abc123"
    );
    // Country TLD (de.indeed.com) is covered by the `.indeed.com` suffix match.
    assert_eq!(
        normalize_job_url("https://de.indeed.com/viewjob?jk=xyz"),
        "https://de.indeed.com/viewjob?jk=xyz"
    );
    // Same job, tracking-only query differences → SAME key (jk retained, the rest
    // dropped). Param ORDER must not matter — allowlist order is authoritative.
    assert_eq!(
        normalize_job_url("https://indeed.com/viewjob?jk=abc&from=serp&utm_source=x"),
        normalize_job_url("https://indeed.com/viewjob?utm_campaign=y&jk=abc")
    );
    assert_eq!(
        normalize_job_url("https://indeed.com/viewjob?jk=abc&from=serp&utm_source=x"),
        "https://indeed.com/viewjob?jk=abc"
    );
    // Path-based URL on a non-allowlisted host is UNCHANGED: the whole query is still
    // dropped (LinkedIn puts the id in the path, so it is unaffected by this fix).
    assert_eq!(
        normalize_job_url("https://www.linkedin.com/jobs/view/12345?trk=abc&refId=z"),
        "https://linkedin.com/jobs/view/12345"
    );
    // A non-Indeed host never retains a query param, even a `jk` lookalike.
    assert_eq!(
        normalize_job_url("https://acme.example/viewjob?jk=abc"),
        "https://acme.example/viewjob"
    );
}

/// Companion to the `canonical_xing_*`/`canonical_stepstone_*` tests in
/// `scraping::scrape_url::tests` (PR 7): both hosts put the job id in the PATH, so
/// neither has an entry in `identifying_query_params` — the whole query must be
/// dropped, not just the tracking param, via `retain_identifying_params` seeing an
/// empty allowlist for the host. Pinned against the real detail-URL shapes
/// (including their tracking params, Xing `?ijt=`/StepStone `?rltr=`) captured
/// live during PR 7's browser probe, so this fails if either host ever gains a
/// query-param allowlist entry without an accompanying deliberate decision, or if
/// `retain_identifying_params`/`identifying_query_params` regress to leaking an
/// unlisted host's query through.
#[test]
fn xing_and_stepstone_tracking_query_is_dropped_entirely() {
    assert_eq!(
        normalize_job_url(
            "https://www.xing.com/jobs/berlin-senior-software-engineer-155853218?ijt=jb_55"
        ),
        "https://xing.com/jobs/berlin-senior-software-engineer-155853218"
    );
    assert_eq!(
        normalize_job_url(
            "https://www.stepstone.de/stellenangebote--Software-Engineer-m-w-d-Distribution-Berlin-GEMA-Gesellschaft-fuer-musik-Auffuehrungs-und-mechan-Vervielfaeltigungsrechte--14009455-inline.html?rltr=1_1_25_seorl_m_0_0_0_0_0_0"
        ),
        "https://stepstone.de/stellenangebote--software-engineer-m-w-d-distribution-berlin-gema-gesellschaft-fuer-musik-auffuehrungs-und-mechan-vervielfaeltigungsrechte--14009455-inline.html"
    );
}
