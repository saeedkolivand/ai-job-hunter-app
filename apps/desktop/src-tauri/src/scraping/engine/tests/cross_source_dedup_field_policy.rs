//! TRUST PR E stage 1: cross-source dedup — the field-level upgrade policy
//! (`dedup_cross_source`): description-length tiebreak, `posted_at`
//! fill-without-clobbering, and `extra`-map union.

use super::super::dedup::dedup_cross_source;
use super::super::*;

// ── TRUST PR E stage 1: cross-source dedup ────────────────────────────────────
//
// `dedup_cross_source` is the single cross-board pass. The pure tests pin the
// survivor policy (field-level upgrade, incumbent identity NEVER swapped)
// directly; the engine-seam test proves it is wired into
// `scrape_boards_with_resolver` and that per-board summary counts stay
// as-fetched (only the aggregated result set is deduped).

/// Minimal `JobPosting` builder for the pure dedup tests — only the fields the
/// canonical key + survivor policy read (url/title/company/description/extra).
pub(super) fn dedup_posting(
    source: &str,
    url: &str,
    title: &str,
    company: &str,
    description: Option<&str>,
) -> JobPosting {
    dedup_posting_with_extra(source, url, title, company, description, &[])
}

/// Like [`dedup_posting`] but with an explicit `extra` map, for the union tests.
fn dedup_posting_with_extra(
    source: &str,
    url: &str,
    title: &str,
    company: &str,
    description: Option<&str>,
    extra: &[(&str, serde_json::Value)],
) -> JobPosting {
    JobPosting {
        id: format!("{source}:{title}"),
        external_id: None,
        title: title.to_string(),
        company: company.to_string(),
        location: None,
        url: url.to_string(),
        source: source.to_string(),
        description: description.map(str::to_string),
        requirements: None,
        posted_at: None,
        captured_at: 0,
        extra: extra
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect(),
    }
}

#[test]
fn dedup_cross_source_keeps_distinct_hacker_news_job_ids_apart() {
    // `boards::ycombinator` falls back to `news.ycombinator.com/item?id=<id>`
    // whenever an HN job story carries no external url — the id lives in the
    // QUERY. `normalize_job_url` drops the whole query for any host without an
    // allowlist entry, so every such posting used to normalize to the bare
    // `/item` path and collapse into one row.
    let out = dedup_cross_source(vec![
        dedup_posting(
            "ycombinator",
            "https://news.ycombinator.com/item?id=44100001",
            "Backend Engineer",
            "Acme",
            None,
        ),
        dedup_posting(
            "ycombinator",
            "https://news.ycombinator.com/item?id=44100002",
            "Frontend Engineer",
            "Beta",
            None,
        ),
    ]);
    assert_eq!(out.len(), 2, "distinct HN job ids must not collapse");

    // Tracking params on the SAME id must still collapse — the allowlist keeps
    // only `id`, so this is the behaviour the allowlist exists to preserve.
    let out = dedup_cross_source(vec![
        dedup_posting(
            "ycombinator",
            "https://news.ycombinator.com/item?id=44100001",
            "Backend Engineer",
            "Acme",
            None,
        ),
        dedup_posting(
            "ycombinator",
            "https://news.ycombinator.com/item?id=44100001&utm_source=x",
            "Backend Engineer",
            "Acme",
            None,
        ),
    ]);
    assert_eq!(out.len(), 1, "the same HN id must still dedup");
}

#[test]
fn dedup_cross_source_upgrades_description_and_extra_but_keeps_incumbent_identity() {
    // Same job, two boards: "aggregator" FIRST (incumbent) — truncated snippet,
    // but carries the salary fields Adzuna scrapes into `extra`; "board" SECOND
    // (challenger) — same canonical URL, full description, no salary, but a
    // `remote` flag the aggregator lacks. This is the exact regression scenario
    // a whole-struct replace would break: a direct board winning on description
    // length must not delete the incumbent's salary.
    let input = vec![
        dedup_posting_with_extra(
            "aggregator",
            "https://www.acme.example/jobs/42?utm_source=x",
            "Staff Engineer",
            "Acme",
            Some("snippet"),
            &[
                ("salaryMin", serde_json::json!(100_000)),
                ("salaryMax", serde_json::json!(140_000)),
                ("salaryCurrency", serde_json::json!("USD")),
            ],
        ),
        dedup_posting_with_extra(
            "board",
            "https://acme.example/jobs/42",
            "Staff Engineer",
            "Acme",
            Some("a much longer full description that beats the snippet"),
            &[
                // Overlapping key with a DIFFERENT value — the incumbent's
                // non-empty value must win, not be overwritten.
                ("salaryCurrency", serde_json::json!("EUR")),
                ("remote", serde_json::json!(true)),
            ],
        ),
    ];
    let out = dedup_cross_source(input);
    assert_eq!(
        out.len(),
        1,
        "same canonical URL across boards collapses to one"
    );

    // Incumbent identity (board attribution / url / id) is NEVER swapped, even
    // though the challenger's description wins.
    assert_eq!(
        out[0].source, "aggregator",
        "incumbent board attribution must be kept, not overwritten by the challenger"
    );
    assert_eq!(
        out[0].id, "aggregator:Staff Engineer",
        "incumbent id must be kept"
    );
    assert_eq!(
        out[0].url, "https://www.acme.example/jobs/42?utm_source=x",
        "incumbent url must be kept"
    );

    // Description IS upgraded — the challenger's is longer.
    assert_eq!(
        out[0].description.as_deref(),
        Some("a much longer full description that beats the snippet"),
        "description must upgrade to the challenger's longer text"
    );

    // Extra is UNIONED, never wholesale replaced: the incumbent's salary fields
    // (which only it had) survive; the incumbent's non-empty overlapping key
    // wins over the challenger's; a challenger-only key is added.
    assert_eq!(
        out[0].extra.get("salaryMin"),
        Some(&serde_json::json!(100_000)),
        "incumbent-only salaryMin must be retained, not deleted by the challenger winning \
         the description"
    );
    assert_eq!(
        out[0].extra.get("salaryMax"),
        Some(&serde_json::json!(140_000)),
        "incumbent-only salaryMax must be retained"
    );
    assert_eq!(
        out[0].extra.get("salaryCurrency"),
        Some(&serde_json::json!("USD")),
        "incumbent's non-empty salaryCurrency must win over the challenger's differing value"
    );
    assert_eq!(
        out[0].extra.get("remote"),
        Some(&serde_json::json!(true)),
        "a challenger-only key must be unioned in"
    );
}

#[test]
fn dedup_cross_source_backfills_posted_at_but_never_clobbers_a_known_date() {
    // Mirrors `dedup_cross_source_upgrades_description_and_extra_but_keeps_incumbent_identity`
    // for `posted_at`: an aggregator hit WITH a publish date, deduped against a
    // dateless direct-board hit for the same canonical key, must not lose the
    // date permanently once collapsed to one row.
    let mut dateless_incumbent = dedup_posting(
        "board",
        "https://acme.example/jobs/42",
        "Staff Engineer",
        "Acme",
        Some("short"),
    );
    dateless_incumbent.posted_at = None;

    let mut dated_challenger = dedup_posting(
        "aggregator",
        "https://www.acme.example/jobs/42?utm_source=x",
        "Staff Engineer",
        "Acme",
        Some("short"),
    );
    dated_challenger.posted_at = Some(1_700_000_000_000);

    let out = dedup_cross_source(vec![dateless_incumbent, dated_challenger]);
    assert_eq!(out.len(), 1, "same canonical key collapses to one");
    assert_eq!(
        out[0].posted_at,
        Some(1_700_000_000_000),
        "a dateless incumbent must backfill the challenger's posted_at, not lose it \
         permanently once collapsed"
    );

    // The other direction: an incumbent that already has a date keeps its OWN
    // date — a challenger's date (or lack of one) never overwrites a known one.
    let mut dated_incumbent = dedup_posting(
        "aggregator",
        "https://www.acme.example/jobs/43?utm_source=x",
        "Senior Engineer",
        "Acme",
        Some("short"),
    );
    dated_incumbent.posted_at = Some(1_650_000_000_000);

    let mut dateless_challenger = dedup_posting(
        "board",
        "https://acme.example/jobs/43",
        "Senior Engineer",
        "Acme",
        Some("a much longer full description that beats the snippet"),
    );
    dateless_challenger.posted_at = None;

    let out = dedup_cross_source(vec![dated_incumbent, dateless_challenger]);
    assert_eq!(out.len(), 1);
    assert_eq!(
        out[0].posted_at,
        Some(1_650_000_000_000),
        "an incumbent's known posted_at must survive a challenger with no date, \
         even when the challenger wins the description upgrade"
    );
}
