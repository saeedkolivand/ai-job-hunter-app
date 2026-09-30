use super::super::*;
use super::support::*;

// ── Adzuna country allowlist ──────────────────────────────────────────────────

/// Every code in ADZUNA_SUPPORTED_COUNTRIES must be accepted by `adzuna_supports_country`.
#[test]
fn adzuna_supported_countries_all_accepted() {
    for &cc in ADZUNA_SUPPORTED_COUNTRIES {
        assert!(
            adzuna_supports_country(cc),
            "country '{cc}' is in the allowlist but adzuna_supports_country returned false"
        );
    }
}

/// Codes not in the allowlist must be rejected (case-sensitive — country is
/// lowercased at the `AggregatorScraper::search` call site, line ~504).
#[test]
fn adzuna_unsupported_countries_rejected() {
    for cc in &["xx", "yy", "kp", "ir", "ru", "cn", "jp", "GB", "US"] {
        assert!(
            !adzuna_supports_country(cc),
            "'{cc}' should not be in the Adzuna allowlist"
        );
    }
}

/// Every code in ADZUNA_SUPPORTED_COUNTRIES must resolve to a currency — an
/// unmapped supported country would silently drop the salary's currency.
#[test]
fn adzuna_currency_map_covers_every_supported_country() {
    for &cc in ADZUNA_SUPPORTED_COUNTRIES {
        assert!(
            adzuna_currency_for_country(cc).is_some(),
            "country '{cc}' is Adzuna-supported but has no currency mapping"
        );
    }
}

/// A handful of country → ISO-4217 currency cases, plus an unsupported country
/// mapping to `None` (the salary answer then falls back to a web lookup).
#[test]
fn adzuna_currency_for_country_maps_known_codes() {
    assert_eq!(adzuna_currency_for_country("us"), Some("USD"));
    assert_eq!(adzuna_currency_for_country("gb"), Some("GBP"));
    assert_eq!(adzuna_currency_for_country("de"), Some("EUR"));
    assert_eq!(adzuna_currency_for_country("pl"), Some("PLN"));
    assert_eq!(adzuna_currency_for_country("ca"), Some("CAD"));
    assert_eq!(adzuna_currency_for_country("xx"), None);
}

fn adzuna_job(salary_min: Option<f64>, salary_max: Option<f64>) -> AdzunaJob {
    AdzunaJob {
        id: "1".to_string(),
        title: "Engineer".to_string(),
        company: None,
        location: None,
        redirect_url: "https://example.com/job/1".to_string(),
        description: None,
        created: None,
        salary_min,
        salary_max,
    }
}

/// A known salary + a supported country writes both the amount and the derived
/// ISO-4217 currency into `extra`.
#[test]
fn adzuna_job_to_posting_writes_salary_and_currency() {
    let posting = adzuna_job_to_posting(adzuna_job(Some(70_000.0), Some(90_000.0)), "de", 0);
    assert_eq!(
        posting.extra.get("salaryMin").and_then(|v| v.as_f64()),
        Some(70_000.0)
    );
    assert_eq!(
        posting.extra.get("salaryMax").and_then(|v| v.as_f64()),
        Some(90_000.0)
    );
    assert_eq!(
        posting.extra.get("salaryCurrency").and_then(|v| v.as_str()),
        Some("EUR")
    );
}

/// No salary at all → no salary/currency keys (an orphan currency with no
/// amount would be meaningless).
#[test]
fn adzuna_job_to_posting_omits_currency_when_no_salary() {
    let posting = adzuna_job_to_posting(adzuna_job(None, None), "de", 0);
    assert!(!posting.extra.contains_key("salaryMin"));
    assert!(!posting.extra.contains_key("salaryMax"));
    assert!(!posting.extra.contains_key("salaryCurrency"));
}

/// A known salary from a country with no currency mapping still keeps the
/// amount but omits the currency — graceful degradation, not a dropped salary.
#[test]
fn adzuna_job_to_posting_keeps_salary_without_currency_for_unmapped_country() {
    let posting = adzuna_job_to_posting(adzuna_job(Some(50_000.0), None), "xx", 0);
    assert_eq!(
        posting.extra.get("salaryMin").and_then(|v| v.as_f64()),
        Some(50_000.0)
    );
    assert!(!posting.extra.contains_key("salaryCurrency"));
}

/// Empty country string resolves to "de" (the Adzuna default) and must pass
/// the allowlist check. Calls the real `AdzunaProvider::search` with an empty
/// country string to prove the production `if country.is_empty() { "de" }` guard
/// fires and that the resulting error is NOT the allowlist-rejection error.
#[tokio::test]
async fn adzuna_empty_country_resolves_to_supported_de() {
    let p = AdzunaProvider {
        app_id: Some("fake-id".to_string()),
        app_key: Some("fake-key".to_string()),
        note_sink: None,
        base_url: ADZUNA_BASE_URL.to_string(),
    };
    // Empty country → production code resolves to "de" → passes allowlist → fails
    // downstream at the network/auth layer (no real keys), NOT at country validation.
    let result = p
        .search("engineer", "Berlin", "", false, None, None, make_token())
        .await;
    let e = result.unwrap_err();
    let msg = e.to_string();
    assert!(
        !msg.contains("not in Adzuna's supported market list"),
        "empty country must resolve to 'de' and pass the allowlist; \
         got allowlist-rejection error instead: {msg}"
    );
}

/// Unsupported country + JSearch configured → JSearch used (transparent fallback).
#[tokio::test]
async fn unsupported_country_with_jsearch_falls_back_to_jsearch() {
    let jsearch_posting = sample_posting("js1", "jsearch");

    // AdzunaProvider::search returns Err for unsupported countries; simulate that
    // with a FakeProvider that errors, which is the exact path AdzunaProvider takes.
    let providers_with_adzuna_err: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::err(
            "adzuna",
            "adzuna: country 'xx' is not in Adzuna's supported market list",
        )),
        Box::new(FakeProvider::ok("jsearch", vec![jsearch_posting.clone()])),
    ];

    let result = search_with_providers(
        &providers_with_adzuna_err,
        "engineer",
        "Seoul",
        "xx",
        false,
        None,
        SearchBudget::items_only(100),
        make_token(),
    )
    .await
    .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].external_id, jsearch_posting.external_id);
}

/// Unsupported country + Adzuna configured + NO JSearch → diagnostic Err,
/// not silent Ok(empty). This is the key UX regression test.
#[tokio::test]
async fn unsupported_country_no_jsearch_returns_diagnostic_err() {
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::err(
            "adzuna",
            "adzuna: country 'xx' is not in Adzuna's supported market list",
        )),
        Box::new(FakeProvider::unconfigured("jsearch")),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "Seoul",
        "xx",
        false,
        None,
        SearchBudget::items_only(100),
        make_token(),
    )
    .await;

    assert!(
        result.is_err(),
        "unsupported country with no JSearch fallback must return Err, not silent Ok(empty)"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("adzuna:")
            && msg.contains("'xx'")
            && msg.contains("not in Adzuna's supported market list"),
        "diagnostic error must name the provider ('adzuna:'), the country code (\"'xx'\"), \
         and the supported-market-list phrase; got: {msg}"
    );
}

/// Supported country → Adzuna used normally (allowlist does not interfere).
#[tokio::test]
async fn supported_country_uses_adzuna_normally() {
    let adzuna_posting = sample_posting("de1", "adzuna");
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::ok("adzuna", vec![adzuna_posting.clone()])),
        Box::new(FakeProvider::err("jsearch", "should not be called")),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "Berlin",
        "de",
        false,
        None,
        SearchBudget::items_only(100),
        make_token(),
    )
    .await
    .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].external_id, adzuna_posting.external_id);
}

/// Neither configured + unsupported country → keyless-empty (no keys = no
/// diagnostic; the user hasn't set up any provider at all).
#[tokio::test]
async fn unsupported_country_no_keys_returns_keyless_empty() {
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::unconfigured("adzuna")),
        Box::new(FakeProvider::unconfigured("jsearch")),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "Seoul",
        "xx",
        false,
        None,
        SearchBudget::items_only(100),
        make_token(),
    )
    .await
    .unwrap();

    assert!(
        result.is_empty(),
        "no keys at all must still return keyless-empty (no diagnostic needed)"
    );
}
