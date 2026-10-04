use super::*;

// The request must deserialize from the camelCase wire shape the renderer
// sends (`url`/`description`). Pins the serde contract without an AppHandle.
#[test]
fn update_description_request_deserializes_camel_case() {
    let json = r#"{"url":"https://example.com/jobs/1","description":"full text"}"#;
    let req: ScrapeUpdateDescriptionRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.url, "https://example.com/jobs/1");
    assert_eq!(req.description, "full text");
}

#[test]
fn validate_rejects_every_invalid_input_with_a_validation_error() {
    let too_long = "x".repeat(MAX_DESCRIPTION_LEN + 1);
    let ok_url = "https://example.com/jobs/1";
    // (url, description, why it must be a validation error)
    for (url, description, why) in [
        ("", "text", "empty url must be a validation error"),
        (
            "   ",
            "text",
            "whitespace-only url must be a validation error",
        ),
        (
            "javascript:alert(1)",
            "text",
            "a non-http(s) scheme must normalize to empty and be rejected",
        ),
        // ── review round 2 (issue #1106 follow-up): schemeless input must be
        // rejected, not silently treated as a "valid" url ──────────────────
        // `normalize_job_url("job-1")` returns `"job-1"` unchanged (no scheme to
        // strip), so without an explicit scheme check a stale caller sending the
        // pre-rename `id` shape would validate and then miss both stores as a
        // silent, honest-looking `data: false`.
        (
            "job-1",
            "text",
            "a schemeless bare token must be rejected up front, not normalized \
             unchanged and looked up as if it were a valid url",
        ),
        // This particular shape happens to already be caught upstream (its
        // `greenhouse:` prefix parses as an explicit non-http(s) scheme, so
        // `normalize_job_url` alone already neutralizes it to ""); pinned
        // anyway as a regression guard on the exact shape called out in
        // review, alongside the schemeless-bare-token case above which the
        // NEW explicit-scheme check is what actually catches.
        (
            "greenhouse:12345",
            "text",
            "a stale caller sending the OLD board-synthetic id format must get a \
             validation error, not a silent honest-looking data:false",
        ),
        (
            ok_url,
            &too_long,
            "a description past the cap must be rejected, not truncated",
        ),
        // ── review round 3 (issue #1106 follow-up, MEDIUM/data-loss): an empty
        // description must be rejected up front, not silently wiped into every
        // matching row across both stores ───────────────────────────────────
        (
            ok_url,
            "",
            "an empty description must be a validation error, not a silent wipe",
        ),
        (
            ok_url,
            "   ",
            "a whitespace-only description must be a validation error too",
        ),
    ] {
        assert!(
            matches!(
                validate_update_description(url, description),
                Err(AppError::Validation(_))
            ),
            "{why}"
        );
    }
}

#[test]
fn validate_accepts_valid_input_and_normalizes_the_url() {
    // At-cap is allowed (boundary): only strictly-over-cap is rejected.
    let at_cap = "x".repeat(MAX_DESCRIPTION_LEN);
    let normalized =
        validate_update_description("  HTTPS://Example.com/Jobs/1/?utm_source=x  ", &at_cap)
            .expect("a normalizable url with an at-cap description must validate");
    assert_eq!(
        normalized, "https://example.com/jobs/1",
        "the returned value is the NORMALIZED url (lowercase host, no trailing slash, \
             tracking params dropped), reused as-is for both stores"
    );
}

// ── review round 2 (issue #1106 follow-up, HIGH): the write-back identity
// must canonicalize BEFORE normalizing, exactly like
// `extension_bridge::agent_read::job_resource` does, so a board-specific
// search/SPA-view url resolves to the same key a `job`/`answers.save`
// read would use ──────────────────────────────────────────────────────

#[test]
fn validate_canonicalizes_a_linkedin_search_view_url_to_the_same_identity_job_resource_uses() {
    let search_view = "https://www.linkedin.com/jobs/search/?currentJobId=4185657072";
    let canonical_view = "https://www.linkedin.com/jobs/view/4185657072";

    let from_search = validate_update_description(search_view, "text")
        .expect("a recognised LinkedIn url must validate");
    let from_canonical = validate_update_description(canonical_view, "text")
        .expect("the canonical view url must validate");

    assert_eq!(
        from_search, from_canonical,
        "the search-view and canonical-view urls for the SAME job must normalize \
             to the identical identity — previously the search-view url normalized to \
             .../jobs/search with the id dropped entirely, matching neither store"
    );
    assert_eq!(
        from_search, "https://linkedin.com/jobs/view/4185657072",
        "must land on the canonical /jobs/view/<id> shape, not the raw search path"
    );
}

// ── either_store_updated (issue #1106 — the two-store OR semantics) ──────

#[test]
fn either_store_updated_is_true_unless_neither_store_matched() {
    for (cache_hit, found_jobs_updated, expected, case) in [
        (true, 0, true, "only the cache matched"),
        (false, 1, true, "only found_jobs matched"),
        (true, 2, true, "both matched"),
        (false, 0, false, "neither matched"),
    ] {
        assert_eq!(
            either_store_updated(cache_hit, found_jobs_updated),
            expected,
            "{case}"
        );
    }
}
