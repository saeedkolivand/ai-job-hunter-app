use super::*;

#[test]
fn test_smartrecruiters_scraper_id() {
    let scraper = SmartRecruitersScraper;
    assert_eq!(scraper.id(), "smartrecruiters");
}

#[test]
fn test_smartrecruiters_scraper_display_name() {
    let scraper = SmartRecruitersScraper;
    assert_eq!(scraper.display_name(), "SmartRecruiters");
}

#[test]
fn test_smartrecruiters_scraper_mode() {
    let scraper = SmartRecruitersScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_location_struct_fields() {
    let location = Location {
        city: Some("Berlin".to_string()),
        country: Some("Germany".to_string()),
        remote: Some(true),
        hybrid: Some(false),
    };
    assert_eq!(location.city, Some("Berlin".to_string()));
    assert_eq!(location.remote, Some(true));
    assert_eq!(location.hybrid, Some(false));
}

#[test]
fn test_location_struct_defaults() {
    let location = Location {
        city: None,
        country: None,
        remote: None,
        hybrid: None,
    };
    assert!(location.city.is_none());
    assert!(location.remote.is_none());
    assert!(location.hybrid.is_none());
}

// ---------------------------------------------------------------------------
// smartrecruiters_work_type — precedence + the exact-partition rule
// ---------------------------------------------------------------------------

#[test]
fn work_type_hybrid_wins_over_remote() {
    assert_eq!(
        smartrecruiters_work_type(Some(true), Some(true)),
        Some(WorkType::Hybrid)
    );
}

#[test]
fn work_type_remote_when_hybrid_false() {
    assert_eq!(
        smartrecruiters_work_type(Some(true), Some(false)),
        Some(WorkType::Remote)
    );
}

/// Board-specific rule: unlike the generic all-false→Unknown policy, both
/// booleans false means on-site HERE — the partition is exact (367 = 3 + 36 +
/// 328), so there is no undeclared bucket for a company that reports at all.
#[test]
fn work_type_both_false_means_on_site_not_unknown() {
    assert_eq!(
        smartrecruiters_work_type(Some(false), Some(false)),
        Some(WorkType::OnSite)
    );
}

/// Both booleans absent — the field itself was missing from the payload —
/// stays Unknown (writes nothing), unlike the both-false case above.
#[test]
fn work_type_both_absent_is_unknown() {
    assert_eq!(smartrecruiters_work_type(None, None), None);
}

/// Regression for the "absence manufactures a positive declaration" defect
/// (PR #1076 review): `remote:false` with `hybrid` absent must stay
/// `Unknown`, NOT resolve to `OnSite` — `OnSite` requires a positive `false`
/// on BOTH sides (see the doc comment on `smartrecruiters_work_type`). A
/// positive signal on one side (`Some(true)`) still resolves even when the
/// other side is absent — only the negative/absent combination is unsafe.
#[test]
fn work_type_only_remote_present_positive_resolves_negative_stays_unknown() {
    assert_eq!(
        smartrecruiters_work_type(Some(true), None),
        Some(WorkType::Remote),
        "a positive remote signal must still resolve when hybrid is absent"
    );
    assert_eq!(
        smartrecruiters_work_type(Some(false), None),
        None,
        "remote:false with hybrid absent has no positive signal from either \
         side and must NOT be classified OnSite — a tenant (or a future \
         payload change) emitting remote without hybrid must not label \
         every hybrid row on-site"
    );
}

/// Symmetric case: a positive `hybrid` signal resolves alone, but
/// `hybrid:false` with `remote` absent has no positive signal either.
#[test]
fn work_type_only_hybrid_present_positive_resolves_negative_stays_unknown() {
    assert_eq!(
        smartrecruiters_work_type(None, Some(true)),
        Some(WorkType::Hybrid)
    );
    assert_eq!(smartrecruiters_work_type(None, Some(false)), None);
}

/// The dual-write regression (same shape as the Ashby `isRemote`/`workplaceType`
/// defect): `remote:true` co-occurring with `hybrid:true` classifies as Hybrid
/// (`work_type_hybrid_wins_over_remote`), so `extra["remote"]` must be `false`
/// for that shape, not a copy of the raw `remote` boolean — otherwise
/// `location_filter`'s "never drop a remote job" rule would fire for a Hybrid
/// posting.
#[test]
fn declared_remote_flag_follows_the_classifier_not_the_raw_boolean() {
    let hybrid_but_remote_true = smartrecruiters_work_type(Some(true), Some(true));
    assert_eq!(hybrid_but_remote_true, Some(WorkType::Hybrid));
    assert!(
        !smartrecruiters_is_declared_remote(hybrid_but_remote_true),
        "a Hybrid row must not also write extra.remote == true"
    );
    assert!(
        smartrecruiters_is_declared_remote(Some(WorkType::Remote)),
        "a genuinely Remote row must still write extra.remote == true"
    );
    assert!(!smartrecruiters_is_declared_remote(None));
    assert!(!smartrecruiters_is_declared_remote(Some(WorkType::OnSite)));
}

#[test]
fn location_type_param_spelling_has_no_separator() {
    assert_eq!(
        smartrecruiters_location_type_param(WorkType::Remote),
        "REMOTE"
    );
    assert_eq!(
        smartrecruiters_location_type_param(WorkType::Hybrid),
        "HYBRID"
    );
    assert_eq!(
        smartrecruiters_location_type_param(WorkType::OnSite),
        "ONSITE"
    );
}

// ---------------------------------------------------------------------------
// build_list_url — the request-side security gate (PR #1076 review finding:
// this URL builder previously had no test, so reverting the
// `work_type_spec()` call to the raw field would have restored the
// URL-amplification vector with a fully green suite)
// ---------------------------------------------------------------------------

#[test]
fn build_list_url_no_work_types_appends_nothing() {
    let url = build_list_url("acme", "", &[]);
    assert_eq!(
        url,
        "https://api.smartrecruiters.com/v1/companies/acme/postings?limit=100"
    );
    assert!(!url.contains("locationType"));
}

#[test]
fn build_list_url_one_locationtype_per_requested_type_in_order() {
    let url = build_list_url(
        "acme",
        "",
        &[WorkType::Remote, WorkType::Hybrid, WorkType::OnSite],
    );
    assert_eq!(
        url,
        "https://api.smartrecruiters.com/v1/companies/acme/postings?limit=100\
&locationType=REMOTE&locationType=HYBRID&locationType=ONSITE"
    );
}

/// The request-param spelling is `ONSITE` — no hyphen/underscore — which
/// differs from the wire VALUE this board reports back in `location.hybrid`/
/// `location.remote` (booleans, not this string at all). See
/// `smartrecruiters_location_type_param`'s doc.
#[test]
fn build_list_url_uses_exact_onsite_param_spelling() {
    let url = build_list_url("acme", "", &[WorkType::OnSite]);
    assert!(url.ends_with("&locationType=ONSITE"));
    assert!(!url.contains("ON_SITE"));
    assert!(!url.contains("On-site") && !url.contains("on-site"));
}

#[test]
fn build_list_url_keyword_present_places_q_before_locationtype() {
    let url = build_list_url("acme", "backend engineer", &[WorkType::Remote]);
    assert_eq!(
        url,
        "https://api.smartrecruiters.com/v1/companies/acme/postings?limit=100\
&q=backend%20engineer&locationType=REMOTE"
    );
}

/// CWE-770 defense-in-depth: `WorkType` has exactly 3 variants, so
/// de-duplicating a caller-supplied (possibly duplicated) slice must never
/// produce more than 3 `&locationType=` occurrences.
#[test]
fn build_list_url_duplicated_input_yields_at_most_three_occurrences() {
    let url = build_list_url(
        "acme",
        "",
        &[
            WorkType::Remote,
            WorkType::Remote,
            WorkType::Remote,
            WorkType::Hybrid,
        ],
    );
    let occurrences = url.matches("&locationType=").count();
    assert!(
        occurrences <= 3,
        "at most 3 possible WorkType variants, got {occurrences}"
    );
    assert_eq!(
        occurrences, 2,
        "de-duped: REMOTE once (first-seen), HYBRID once"
    );
}

#[test]
fn test_posting_struct_fields() {
    let posting = Posting {
        id: "123".to_string(),
        uuid: Some("abc".to_string()),
        name: "Software Engineer".to_string(),
        location: None,
        released_date: None,
        ref_field: None,
    };
    assert_eq!(posting.id, "123");
    assert_eq!(posting.name, "Software Engineer");
}

#[test]
fn test_smartrecruiters_scraper_mode_partial_eq() {
    let mode = ScraperMode::Http;
    assert_eq!(mode, ScraperMode::Http);
    assert_ne!(mode, ScraperMode::Browser);
}

#[test]
fn test_smartrecruiters_requires_company() {
    assert!(
        SmartRecruitersScraper.requires_company(),
        "SmartRecruiters is an ATS board and must return true for requires_company()"
    );
}

// ---------------------------------------------------------------------------
// normalize_companies — unit tests (network-free)
// ---------------------------------------------------------------------------

#[test]
fn normalize_drops_blank_entries() {
    let input = vec![
        "Visa".to_string(),
        "".to_string(),
        "   ".to_string(),
        "\t".to_string(),
        "IKEA".to_string(),
    ];
    let result = normalize_companies(&input, 20);
    assert_eq!(result, vec!["Visa", "IKEA"]);
}

#[test]
fn normalize_trims_whitespace() {
    let input = vec!["  Visa  ".to_string(), "\tIKEA\n".to_string()];
    let result = normalize_companies(&input, 20);
    assert_eq!(result, vec!["Visa", "IKEA"]);
}

#[test]
fn normalize_dedupes_first_seen_order() {
    let input = vec![
        "Alpha".to_string(),
        "Beta".to_string(),
        "Alpha".to_string(), // duplicate — must be dropped
        "Gamma".to_string(),
        "Beta".to_string(), // duplicate — must be dropped
    ];
    let result = normalize_companies(&input, 20);
    assert_eq!(result, vec!["Alpha", "Beta", "Gamma"]);
}

#[test]
fn normalize_dedupes_after_trim() {
    // "  Alpha  " and "Alpha" are the same after trimming.
    let input = vec!["  Alpha  ".to_string(), "Alpha".to_string()];
    let result = normalize_companies(&input, 20);
    assert_eq!(result, vec!["Alpha"]);
}

#[test]
fn normalize_caps_at_max_20() {
    // SmartRecruiters cap is 20 — 30 entries must be truncated to 20.
    let input: Vec<String> = (0..30).map(|i| format!("company-{i}")).collect();
    let result = normalize_companies(&input, 20);
    assert_eq!(result.len(), 20);
    assert_eq!(result[0], "company-0");
    assert_eq!(result[19], "company-19");
    // Entry 20+ must be absent.
    assert!(!result.contains(&"company-20".to_string()));
}

#[test]
fn normalize_cap_exact_boundary() {
    // Exactly MAX_COMPANIES (20) entries — none should be dropped.
    let input: Vec<String> = (0..20).map(|i| format!("co-{i}")).collect();
    let result = normalize_companies(&input, 20);
    assert_eq!(result.len(), 20);
}

/// SmartRecruiters seed slugs like `BoschGroup` use exact casing —
/// `normalize_companies` must never lowercase it.
#[test]
fn ats_seed_smartrecruiters_casing_survives_normalize_companies() {
    let slugs: Vec<String> = crate::scraping::boards::ats_seed::by_ats("smartrecruiters")
        .map(|e| e.slug.to_string())
        .collect();
    assert!(!slugs.is_empty(), "smartrecruiters must have seed entries");
    let normalized = normalize_companies(&slugs, 20);
    assert!(
        normalized.iter().any(|s| s == "BoschGroup"),
        "casing 'BoschGroup' must survive verbatim; got {normalized:?}"
    );
}

#[test]
fn normalize_empty_input_returns_empty() {
    let result = normalize_companies(&[], 20);
    assert!(result.is_empty());
}

#[test]
fn normalize_all_blanks_returns_empty() {
    let input = vec!["".to_string(), "   ".to_string(), "\n".to_string()];
    let result = normalize_companies(&input, 20);
    assert!(result.is_empty());
}

// ---------------------------------------------------------------------------
// search() — network-free edge cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn empty_companies_returns_empty_without_network() {
    let scraper = SmartRecruitersScraper;
    let input = BoardSearchInput {
        query: String::new(),
        location: None,
        amount: 5,
        pages: 1,
        provider_amount: None,
        date_filter: None,
        job_type: None,
        work_types: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: Vec::new(),
    };
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
        on_truncation: None,
        on_note: None,
    };
    let result = scraper.search(input, ctx).await;
    assert!(result.is_ok(), "empty companies must return Ok, not Err");
    assert!(
        result.unwrap().is_empty(),
        "empty companies must return empty Vec"
    );
}

/// When all company entries are blank or whitespace the list normalises to
/// empty — the scraper produces Ok([]) without attempting any network I/O.
///
/// ponytail: SmartRecruiters does not have an all-fail Err path (it uses
/// continue on list-fetch errors, not first_fetch_error tracking). The
/// normalisation guarantee is covered by the normalize_companies unit tests
/// above.
#[tokio::test]
async fn all_blank_companies_returns_ok_empty() {
    let scraper = SmartRecruitersScraper;
    let input = BoardSearchInput {
        query: String::new(),
        location: None,
        amount: 5,
        pages: 1,
        provider_amount: None,
        date_filter: None,
        job_type: None,
        work_types: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: vec!["".to_string(), "   ".to_string()],
    };
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
        on_truncation: None,
        on_note: None,
    };
    let result = scraper.search(input, ctx).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = SmartRecruitersScraper;
    let input = BoardSearchInput {
        query: String::new(),
        location: None,
        amount: 5,
        pages: 1,
        provider_amount: None,
        date_filter: None,
        job_type: None,
        work_types: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: vec!["Visa".to_string()], // confirmed live: 7 postings via SmartRecruiters API
    };
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
        on_truncation: None,
        on_note: None,
    };
    let results = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        scraper.search(input, ctx),
    )
    .await
    .expect("live search timed out");
    assert!(results.is_ok(), "search failed: {:?}", results.err());
    let postings = results.unwrap();
    assert!(!postings.is_empty(), "expected >=1 posting, got 0");
    let first = &postings[0];
    assert!(!first.title.is_empty(), "first posting has empty title");
    assert!(!first.url.is_empty(), "first posting has empty url");
    println!("smartrecruiters: {} results", postings.len());
    println!("first: {:?}", first.title);
}
