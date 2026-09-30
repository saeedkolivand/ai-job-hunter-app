use super::super::*;

// ── ADR-029 cross-board clustering acceptance (fixture → REAL board parsers) ──
//
// The SAME real-world job, ingested through two DIFFERENT real board parsers
// with two DIFFERENT wire shapes, must collapse into ONE cluster with both
// source refs and the canonical-preference winner (ADR-029 §c–e). The hand-built
// `cluster::assign_clusters` unit tests assert the join math directly; this test
// closes the acceptance gap by DERIVING the `ClusterInput`s from real parser
// output, so a drift in a board parser's title/company/source mapping (which the
// normalized-company blocker + title trigram join key off) is caught at the
// clustering boundary too. No network — fixtures only.
//
// Shapes exercised:
//   * direct board, structured-markup feed WITH full text →
//     `germantechjobs::parse_feed` (the repo's designated fixture-driven board
//     parser — extracted from `search()` precisely so tests drive it without a
//     network call). German-decorated title + legal-suffix company.
//   * aggregator JSON (Adzuna search API) WITHOUT a description →
//     `adzuna_job_to_posting(serde_json::from_str::<AdzunaResp>(..))`.
//
// NOTE: LinkedIn's own HTML-card parser (the matrix's named "HTML-card shape")
// could NOT stand in for the direct board here — it is embedded inside the async
// `search_guest` HTTP method with no pure, isolable `parse(html) -> Vec<JobPosting>`
// seam, so it is not fixture-drivable without a production testability refactor
// (flagged as a fast-follow). The direct-board-full-text vs aggregator-JSON
// distinction the acceptance turns on is fully exercised by the germantechjobs
// (direct, full-text) + Adzuna (aggregator, snippet-less) pair below.

const GTJ_ACCEPTANCE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<jobs>
<job id="gtj-rust-001">
  <id><![CDATA[gtj-rust-001]]></id>
  <title><![CDATA[Senior Rust Developer (m/w/d) – Berlin]]></title>
  <company><![CDATA[Acme GmbH]]></company>
  <location><![CDATA[Berlin, Deutschland]]></location>
  <url><![CDATA[https://germantechjobs.de/jobs/acme-senior-rust-developer]]></url>
  <pubdate><![CDATA[15.03.2024]]></pubdate>
  <description><![CDATA[<p>Join <b>Acme</b> to build high-performance systems in Rust.</p>]]></description>
</job>
</jobs>"#;

const ADZUNA_ACCEPTANCE_JSON: &str = r#"{
  "results": [
    {
      "id": "4172839571",
      "title": "Senior Rust Developer",
      "company": { "display_name": "Acme" },
      "location": { "display_name": "Berlin, Germany" },
      "redirect_url": "https://www.adzuna.de/details/4172839571",
      "created": "2024-03-15T09:00:00Z"
    }
  ]
}"#;

#[test]
fn cross_board_same_job_from_gtj_feed_and_adzuna_json_forms_one_cluster() {
    let now = 1_700_000_000_000_i64;

    // 1) Parse each fixture through its REAL board parser (no network).
    let mut gtj = crate::scraping::boards::germantechjobs::parse_feed(
        GTJ_ACCEPTANCE_XML,
        "germantechjobs",
        now,
    );
    assert_eq!(
        gtj.len(),
        1,
        "germantechjobs fixture must parse to exactly one posting"
    );
    let gtj = gtj.remove(0);

    let adz_resp: AdzunaResp =
        serde_json::from_str(ADZUNA_ACCEPTANCE_JSON).expect("adzuna fixture must deserialize");
    let adz_job = adz_resp
        .results
        .into_iter()
        .next()
        .expect("adzuna fixture must contain one result");
    let adz = adzuna_job_to_posting(adz_job, "de", now);

    // Sanity: the parsers produced the two shapes clustering keys off — a direct
    // board with full text vs an aggregator copy with none.
    assert_eq!(gtj.source, "germantechjobs");
    assert!(
        gtj.description
            .as_deref()
            .is_some_and(|d| !d.trim().is_empty()),
        "the direct-board posting must carry a description"
    );
    assert_eq!(adz.source, "aggregator");
    assert!(
        adz.description.as_deref().unwrap_or("").trim().is_empty(),
        "the aggregator copy must have no description in this fixture"
    );

    // 2) Distinct board urls → distinct canonical keys: this is a FUZZY cluster
    //    join, NOT a stage-1 exact-key dedup collapse.
    let gtj_key =
        crate::scraping::boards::common::canonical_job_key(&gtj.url, &gtj.title, &gtj.company);
    let adz_key =
        crate::scraping::boards::common::canonical_job_key(&adz.url, &adz.title, &adz.company);
    assert_ne!(
        gtj_key, adz_key,
        "distinct board urls must yield distinct canonical keys (fuzzy join, not exact dedup)"
    );

    // Map through the SAME production seam the manual-scrape ingest uses
    // (`cluster::posting_cluster_input`) so a drift in that mapping is caught
    // here too — no hand-rolled local copy. The aggregator copy is placed FIRST
    // so a passing canonical assertion proves the ADR-029 §e preference order
    // ran, not mere input order.
    let inputs = vec![
        crate::scraping::cluster::posting_cluster_input(&adz, None, None),
        crate::scraping::cluster::posting_cluster_input(&gtj, None, None),
    ];
    let out =
        crate::scraping::cluster::assign_clusters(inputs, &std::collections::HashSet::new(), &[]);

    // 3a) One cluster, two source refs.
    assert_eq!(
        out[0].cluster_id, out[1].cluster_id,
        "the same job on two boards must share one cluster"
    );
    let members = &out[1].members; // canonical row carries the full member list
    assert_eq!(members.len(), 2, "the cluster must list both source refs");
    let boards: std::collections::HashSet<&str> =
        members.iter().filter_map(|m| m.board.as_deref()).collect();
    assert!(
        boards.contains("germantechjobs") && boards.contains("aggregator"),
        "both board source refs must be present, got: {boards:?}"
    );

    // 3b) Canonical-preference winner: has_description > direct board > aggregator
    //     → the germantechjobs row wins even though the aggregator copy was first.
    assert_eq!(
        out[1].cluster_id, gtj_key,
        "cluster id is the canonical member's key"
    );
    assert!(
        out[1].canonical,
        "the described direct-board row is the cluster canonical"
    );
    assert!(
        !out[0].canonical,
        "the aggregator copy is a non-canonical member"
    );
}
