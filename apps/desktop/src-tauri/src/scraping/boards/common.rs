//! Shared helpers for company-scoped ATS board scrapers (Ashby, BambooHR,
//! Breezy, Greenhouse, Pinpoint, Rippling, SmartRecruiters). Extracted from
//! 7 byte-identical copies of `normalize_companies` and 2 byte-identical
//! copies of `is_https_url` — see `.claude/scratch/scraping-followups.md`.
//! Per-board `is_valid_<board>_slug` validators are mostly NOT here: most
//! genuinely differ per board (DNS-label vs URL-path-segment rules, subdomain
//! vs path-segment interpolation — e.g. Personio, Recruitee, Rippling,
//! Workable) by design. The exception is [`is_valid_dns_label_slug`]: BambooHR,
//! Breezy, and Pinpoint all validate a subdomain-interpolated slug with the
//! exact same DNS-label character-set rule, so that one shape is extracted
//! here and shared instead of kept as 3 more byte-identical copies.

/// Trim, drop blanks, dedupe (first-seen order), and cap to `max`.
/// Extracted so the normalisation logic can be unit-tested without network.
pub(crate) fn normalize_companies(input: &[String], max: usize) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    input
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && seen.insert(s.clone()))
        .take(max)
        .collect()
}

/// Require a well-formed `https:` URL with a host and no embedded userinfo.
/// Used by boards (Pinpoint, Breezy) whose response `url` field is
/// display-only (used as a dedup key, not fetched by us), so this is a cheap
/// sanity parse, not a host allowlist — but a
/// `https://user:pass@evil.example/…` URL is still a mild phishing vector on
/// a link the user opens, so userinfo is rejected outright.
pub(crate) fn is_https_url(url: &str) -> bool {
    reqwest::Url::parse(url)
        .map(|u| {
            u.scheme() == "https"
                && u.host_str().is_some()
                && u.username().is_empty()
                && u.password().is_none()
        })
        .unwrap_or(false)
}

/// Validate that a company slug is a single valid DNS hostname label
/// (alphanumeric + hyphen, max 63 chars, no leading/trailing hyphen). Used by
/// boards (BambooHR, Breezy, Pinpoint) that interpolate the slug as a
/// SUBDOMAIN — a slug with dots, slashes, or colons could change the URL
/// authority and redirect the fetch away from the target host (SSRF).
pub(crate) fn is_valid_dns_label_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 63
        && slug.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !slug.starts_with('-')
        && !slug.ends_with('-')
}

/// Whether a board's own location text satisfies the requested location, for the
/// boards that filter client-side.
///
/// `requested` is normally the geocode picker's `"<city>, <country>"` label
/// (`commands::geocoding` builds it precisely "so the UI only ever shows
/// 'City, Country'"), while a board writes its location in its own shape —
/// `"New York, NY"`, `"Munich, Bayern"`, `"Karl-Marx-Allee 1, Berlin"`. Requiring
/// the WHOLE label as one contiguous substring therefore matched nothing and
/// zeroed the board for every picked city, so match SEGMENT-wise instead: any
/// non-empty comma-separated part of the request appearing in `haystack` is
/// enough.
///
/// Conservative by design — it errs toward keeping a posting, mirroring the
/// engine's central [`crate::scraping::engine::location_filter`], which
/// tokenizes the request and keeps on any token hit for the same reason. Both
/// arguments must already be lowercased by the caller.
pub(crate) fn location_matches(requested: &str, haystack: &str) -> bool {
    requested
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .any(|part| haystack.contains(part))
}

/// Client-side query/location filter for boards with no server-side keyword
/// search (The Muse, Comeet). Case-insensitive substring match on
/// `title + company` for `query`; segment-wise case-insensitive match on
/// `location` for `location` (see [`location_matches`]); an empty filter passes
/// everything; both clauses AND-combine. Originally The Muse-local; extracted
/// here once Comeet needed the identical filter instead of a second copy.
pub(crate) fn matches_filters(
    posting: &crate::scraping::types::JobPosting,
    query: &str,
    location: &str,
) -> bool {
    let q = query.trim().to_lowercase();
    if !q.is_empty() {
        let haystack = format!("{} {}", posting.title, posting.company).to_lowercase();
        if !haystack.contains(&q) {
            return false;
        }
    }

    let loc_filter = location.trim().to_lowercase();
    if !loc_filter.is_empty() {
        let loc = posting.location.as_deref().unwrap_or("").to_lowercase();
        if !location_matches(&loc_filter, &loc) {
            return false;
        }
    }

    true
}

/// Decide whether an ATS multi-company board's fetch loop should fail the
/// whole board. Returns `Some(message)` — ready to wrap in
/// `anyhow::anyhow!` — only when every attempted per-company fetch failed
/// (`successful_fetches == 0` AND at least one real fetch error was
/// recorded). Returns `None` when at least one company succeeded (partial
/// success is kept) OR when no company ever reached a fetch (e.g. every slug
/// was rejected by a pre-fetch validation guard, or the input list was
/// empty) — that is a "nothing to fetch" outcome, not a board failure.
///
/// Pure — no I/O, no board host — so it is directly unit-testable without a
/// mock server. Shared by every ATS board that tracks
/// `successful_fetches`/`first_fetch_error` (Ashby, BambooHR, Breezy,
/// Greenhouse, Lever, Personio, Pinpoint, Recruitee, Rippling, SmartRecruiters,
/// Workable — Personio joined in trust-H, closing the gap where its
/// `fetch_text` XML-feed fetches never fed this shared failure check).
pub(crate) fn ats_all_fetches_failed(
    board_id: &str,
    successful_fetches: usize,
    first_fetch_error: &Option<String>,
) -> Option<String> {
    if successful_fetches > 0 {
        return None;
    }
    first_fetch_error
        .as_ref()
        .map(|error| format!("all {board_id} company fetches failed: {error}"))
}

/// The distinct board-error message for a run whose every company slug was
/// rejected by a pre-fetch validator (e.g. [`is_valid_dns_label_slug`]) before
/// any network call ran. Shared so every ATS board words it identically.
pub(crate) fn ats_all_slugs_invalid_message(board_id: &str, rejected_slugs: usize) -> String {
    format!(
        "all {rejected_slugs} company slug(s) invalid for {board_id} — check the company names in the jobs search form"
    )
}

/// Extends [`ats_all_fetches_failed`] to also surface company slugs that a
/// pre-fetch validation guard rejected before any fetch ran. Those rejects were
/// previously invisible: a run whose every slug was malformed recorded no
/// `successful_fetches` AND no `first_fetch_error`, so `ats_all_fetches_failed`
/// returned `None` and the board reported a silent zero (claude review #597).
/// Used by the ATS boards that validate slugs up front (BambooHR, Breezy,
/// Pinpoint, Rippling, Workable).
///
/// Decision table (given `successful_fetches`, `rejected_slugs`, `first_fetch_error`):
/// - `successful_fetches > 0` → `None` (partial success kept — a rejected or
///   errored remainder is a per-board log concern, not a whole-board failure);
/// - else a real fetch error was recorded → the same
///   `"all {board} company fetches failed: {error}"` message as
///   [`ats_all_fetches_failed`] (a mix of rejects + fetch errors reports the
///   fetch error, the more actionable signal);
/// - else `rejected_slugs > 0` → the distinct
///   [`ats_all_slugs_invalid_message`] ("all N company slug(s) invalid …");
/// - else (nothing attempted, nothing rejected) → `None`.
///
/// Pure — directly unit-testable without a mock server.
pub(crate) fn ats_board_failure(
    board_id: &str,
    successful_fetches: usize,
    rejected_slugs: usize,
    first_fetch_error: &Option<String>,
) -> Option<String> {
    if let Some(msg) = ats_all_fetches_failed(board_id, successful_fetches, first_fetch_error) {
        return Some(msg);
    }
    (successful_fetches == 0 && rejected_slugs > 0)
        .then(|| ats_all_slugs_invalid_message(board_id, rejected_slugs))
}

/// Finalize an ATS multi-company board's `search()`: cancellation ALWAYS wins
/// over a synthesized board error. A cancel that fires right after an invalid
/// slug is rejected — but before a later valid slug is reached — must not be
/// misattributed as "all slugs invalid" (a benign interruption reported as
/// user misconfiguration). This priority is easy to get right per board and
/// easy to silently drop in a copy-paste (it was — see the round-1 review
/// finding this fixed), so it is centralized here: every board that shares
/// this finish step gets the priority for free instead of re-deriving it, and
/// the ONE test on this function pins it for all of them at once.
///
/// Shared by every ATS board using the full `ats_board_failure` shape
/// (BambooHR, Breezy, Personio, Pinpoint, Recruitee, Rippling, Workable —
/// Personio joined in trust-H, replacing the narrower inline
/// `(attempted, rejected_slugs)` pair it used to track on its own with the
/// full `successful_fetches`/`first_fetch_error` shape every other board here
/// already used).
pub(crate) fn ats_finish_search(
    signal: &tokio_util::sync::CancellationToken,
    out: Vec<crate::scraping::types::JobPosting>,
    board_id: &str,
    successful_fetches: usize,
    rejected_slugs: usize,
    first_fetch_error: &Option<String>,
) -> anyhow::Result<Vec<crate::scraping::types::JobPosting>> {
    if signal.is_cancelled() {
        return Ok(out);
    }
    if let Some(message) = ats_board_failure(
        board_id,
        successful_fetches,
        rejected_slugs,
        first_fetch_error,
    ) {
        return Err(anyhow::anyhow!(message));
    }
    Ok(out)
}

/// The ONE informational partial-visibility note (PR D `kind:value` grammar) an
/// ATS multi-company board emits when its run partially degraded but STILL
/// returned a result — the previously log-only anomalies now made visible:
/// - `slugs-invalid:<n>` — SOME (not all) company slugs were rejected pre-fetch
///   (`n` = rejected count). Preferred over `rows-dropped`: an invalid slug is a
///   fixable user input, the more actionable signal.
/// - `rows-dropped:<n>` — SOME rows were dropped by per-row parsing (`rows_to_jobs`
///   schema-drift on individual rows, `n` = total dropped) while the rest parsed.
///
/// Returns `None` when there was no such anomaly, OR when EVERY fetch failed /
/// every slug was rejected (`successful_fetches == 0`) — that whole-board FAILURE
/// is surfaced as an `Err` by [`ats_finish_search`], never as a note. At most ONE
/// token per board per run; `slugs-invalid` wins when both apply.
///
/// Pure — directly unit-testable without a mock server. The caller emits the
/// returned token via `ScrapeContext::report_note`, gated on non-cancellation
/// (a benign interruption reports nothing, mirroring [`ats_finish_search`]).
pub(crate) fn ats_partial_note(
    successful_fetches: usize,
    rejected_slugs: usize,
    rows_dropped: usize,
) -> Option<String> {
    if successful_fetches == 0 {
        return None;
    }
    if rejected_slugs > 0 {
        return Some(format!("slugs-invalid:{rejected_slugs}"));
    }
    (rows_dropped > 0).then(|| format!("rows-dropped:{rows_dropped}"))
}

/// Decide the pagination-failure policy for a page fetch: a page reached with
/// nothing collected yet (typically page 0) propagates the fetch failure as a
/// board error; a later page that fails after some items were already
/// streamed instead stops pagination and keeps the partial result.
///
/// Pure — takes only the already-known collected count, no I/O — so it is
/// directly unit-testable without a mock server. Shared by every paginated
/// board that fails closed on an empty-so-far collection (The Muse,
/// Arbeitnow, Arbeitsagentur).
pub(crate) fn should_propagate_page_error(collected_so_far: usize) -> bool {
    collected_so_far == 0
}

/// App-wide canonical dedup key for a job posting — the single source of truth
/// for "is this the same job?" across every source (the scrape engine's
/// cross-board pass, autopilot's `merge_found_jobs`, and — mirrored in TS at
/// `apps/desktop/src/renderer/features/jobs/lib/canonical-job-key.ts` — the
/// renderer's `mergePostings`). Keeping all three keyed identically is the whole
/// point of trust-program PR E: a job surfaced by two boards collapses to one row
/// and fires one notification, not two or three.
///
/// **Exact algorithm (mirror this precisely in any other language):**
/// 1. `n = normalize_job_url(url)` — the app-wide URL identity
///    ([`crate::applications::normalize_job_url`]): rejects any non-`http(s)`
///    scheme to `""`; lowercases the whole URL; strips a leading `www.` on the
///    host; drops the `#fragment`; drops the query string **except** per-host
///    identifying params (currently only `indeed.com`'s `jk`, emitted in a fixed
///    order); strips a trailing `/` on the path. Empty/blank input → `""`.
/// 2. If `n` is non-empty, the key **is** `n` (URL identity).
/// 3. Otherwise (missing/unusable/non-http URL) fall back to
///    `"{title}\u{1}{company}"` with each side `.trim().to_lowercase()`. The
///    `U+0001` (SOH) separator can't occur in real text, so a title that merely
///    contains the company name can't forge a colliding key — and near-miss
///    titles stay distinct (`"senior rust engineer\u{1}acme"` ≠
///    `"rust engineer\u{1}acme"`).
///
/// Chosen over the aggregator's narrower private `canonical_url` (which only
/// strips LinkedIn query params and is a within-aggregator provider-merge
/// concern): `normalize_job_url` is the genuinely app-wide, well-tested URL
/// identity that autopilot's merge already keys on, so building on it keeps the
/// stages in lockstep instead of diverging on two different URL notions.
///
/// Known limitation (out of scope for stage 1): an aggregator *redirect* URL and
/// the board's *direct* posting URL normalize to different keys, so the same job
/// reached via a redirect is not collapsed here — that needs redirect-chain
/// canonicalization, tracked separately.
///
/// The title+company fallback assumes each board already validated/populated
/// `title` (every registered `Scraper` does); it is not itself a title
/// validator, so postings with all-empty titles/companies collapse to the
/// single `"\u{1}"` key.
///
/// Pure — no I/O — so the key is directly unit-testable.
pub(crate) fn canonical_job_key(url: &str, title: &str, company: &str) -> String {
    let normalized = crate::applications::normalize_job_url(url);
    if !normalized.is_empty() {
        normalized
    } else {
        format!(
            "{}\u{1}{}",
            title.trim().to_lowercase(),
            company.trim().to_lowercase()
        )
    }
}

#[cfg(test)]
mod test;
