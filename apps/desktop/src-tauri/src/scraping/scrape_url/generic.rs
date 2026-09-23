//! Generic-HTML extraction for [`super::parse_from_html`] — everything used
//! when no named board recognised the URL: readability/main-content
//! description rungs, the `<title>`/`<h1>` + meta pass, site-branding suffix
//! removal, and the employer-name heuristics.
//!
//! Split out of the parent when it crossed the R8 hard LOC cap. It is the
//! natural seam: nothing here makes a network request or knows any board's
//! shape — every function is a pure read of already-fetched markup, which is
//! also why every one of them is directly unit-testable.
//!
//! The markup is attacker-controlled (a scraped page, or a DOM the extension
//! captured from one), so the extracted title/company are byte-capped before
//! they can reach a stored Application row.

use scraper::{Html, Selector};

/// Real-readability last-resort description: run `dom_smoothie` (a faithful
/// Rust port of Mozilla's Readability.js) over the whole document, so nav/
/// footer/boilerplate get scored out instead of surviving a naive
/// largest-block guess. `is_probably_readable()` must run before `parse()`
/// (it inspects the un-mutated document; `parse()` mutates it) and gates
/// documents too thin to trust — e.g. a nav-only shell with no real article.
/// Both the pre-check and `parse()` are best-effort: any `Err` (bad URL,
/// `max_elements_to_parse` exceeded, no candidate found) falls through to
/// `None` — the caller then tries `main_content_text` — rather than
/// propagating, since this is an enrichment, not a hard requirement.
// TODO(perf): this runs synchronous CPU parsing on the async executor with no
// `spawn_blocking` (same as every other rung of `parse_from_html`'s generic-HTML
// fallback — all use `Html::parse_document`). Deferred: wrap the whole generic
// fallback in `spawn_blocking` at its async call sites (including the
// extension-bridge Scan path) — a broader refactor, out of scope here.
pub(super) fn readability_content_text(url: &str, html: &str) -> Option<String> {
    // Host only — never log the raw dom_smoothie error, which can embed the
    // scraped `url` (see the `host`-only convention at parse_from_html's own
    // `reqwest::Url::parse` call and the `linkedin` job-id log above).
    let host = reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();
    let cfg = dom_smoothie::Config {
        // `text_content` becomes ready-to-use markdown straight off the
        // cleaned readability DOM — skips a second html_to_markdown pass over
        // `article.content` (which would re-parse already-cleaned HTML).
        text_mode: dom_smoothie::TextMode::Markdown,
        // Defense-in-depth against a hostile/huge page: default `0` means
        // UNLIMITED, letting an attacker-sized document drive an unbounded
        // multi-pass scoring parse (CPU/peak-memory amplification). 4000 is
        // comfortably above any real job posting page (dom_smoothie's own
        // test suite parses a full Wikipedia article — far denser than a job
        // page — under 10,000 with no false-positive `TooManyElements`)
        // while still bounding abuse. Tripping the cap returns
        // `Err(TooManyElements)`, handled by the `Err ⇒ None` arm below —
        // clean fall-through to `main_content_text`.
        max_elements_to_parse: 4000,
        ..Default::default()
    };
    let mut readability = match dom_smoothie::Readability::new(html, Some(url), Some(cfg)) {
        Ok(r) => r,
        Err(_) => {
            log::debug!(
                "[scraping::scrape_url] dom_smoothie::Readability::new failed for host {host}"
            );
            return None;
        }
    };
    if !readability.is_probably_readable() {
        return None;
    }
    match readability.parse() {
        Ok(article) => {
            let text = article.text_content.trim().to_string();
            (!text.is_empty()).then_some(text)
        }
        Err(_) => {
            log::debug!("[scraping::scrape_url] dom_smoothie parse failed for host {host}");
            None
        }
    }
}

/// Largest main-content text block as a FINAL last-resort description (below
/// `readability_content_text`): pick the longest rendered text among `main` /
/// `[role="main"]` / `article`. Kept as the floor for when readability's own
/// pre-check or `parse()` comes back empty — a naive guess beats nothing.
pub(super) fn main_content_text(html: &str) -> Option<String> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(r#"main, [role="main"], article"#).ok()?;
    doc.select(&sel)
        .map(|el| crate::scraping::http::html_to_markdown(&el.inner_html()))
        .filter(|t| !t.trim().is_empty())
        .max_by_key(|t| t.len())
}

pub(super) fn parse_generic_html(html: &str) -> (String, Option<String>) {
    let doc = Html::parse_document(html);
    let title_sel = Selector::parse("title, h1").unwrap();
    let title = doc
        .select(&title_sel)
        .next()
        .map(|e| e.text().collect::<String>().trim().to_string())
        .unwrap_or_default();
    let meta_sel =
        Selector::parse("meta[name=\"description\"], meta[property=\"og:description\"]").unwrap();
    let description = doc
        .select(&meta_sel)
        .next()
        .and_then(|e| e.value().attr("content").map(str::to_string));
    (title, description)
}

/// Byte cap on a title/company pulled out of attacker-controlled captured
/// markup. Both end up stored on an Application row and rendered, so neither
/// may be unbounded; a real job title or employer name is far under this.
pub(super) const GENERIC_FIELD_CAP: usize = 200;

/// Whether `html` embeds a CROSS-ORIGIN `<iframe>` relative to `page_url` —
/// the shape an ATS board takes when a company embeds it in its own careers
/// page (issue #1238).
///
/// The extension captures the TOP-LEVEL document only (no `allFrames`, by
/// deliberate permission design — see `apps/extension/README.md`), so a posting
/// living inside such a frame is simply unreachable, and the generic parser
/// then describes the WRAPPER page instead. This predicate does not prove that
/// happened; it is the corroborating half of a two-part test, and the caller
/// pairs it with "we also failed to extract any description". On its own it
/// would fire on the analytics, video and consent frames that sit on almost
/// every page.
///
/// Same-origin frames, relative `src`s, and the non-document schemes
/// (`about:`/`data:`/`javascript:`) are all ignored — none of them is a
/// third-party board.
pub(crate) fn has_cross_origin_iframe(html: &str, page_url: &str) -> bool {
    let Some(page_host) = reqwest::Url::parse(page_url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_ascii_lowercase))
    else {
        return false;
    };
    let doc = Html::parse_document(html);
    let Ok(sel) = Selector::parse("iframe[src]") else {
        return false;
    };
    doc.select(&sel).any(|el| {
        el.value()
            .attr("src")
            // A relative src resolves to this same origin, so only an
            // absolute URL can be cross-origin.
            .and_then(|src| reqwest::Url::parse(src).ok())
            .and_then(|u| u.host_str().map(str::to_ascii_lowercase))
            .is_some_and(|h| h != page_host)
    })
}

/// The page's own `og:site_name`, when it declares one — the most reliable
/// answer to [`strip_site_suffix`]'s question "is this trailing segment the
/// SITE rather than the employer". Parses the document once more rather than
/// threading it through: this runs once per generic-fallback import, alongside
/// the several parses `parse_generic_html`/`parse_generic_company`/readability
/// already do, and is not on any loop.
pub(super) fn og_site_name(html: &str) -> Option<String> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(r#"meta[property="og:site_name"]"#).ok()?;
    doc.select(&sel)
        .next()
        .and_then(|e| e.value().attr("content"))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Strip ONE trailing `" | <site>"` / `" - <site>"` / `" – <site>"` suffix from a
/// page title, and only when `<site>` actually matches a site signal — the
/// `og:site_name`, or the registrable-ish label of the host (issue #1239:
/// LinkedIn's authenticated job page yields
/// `"Javascript Developer | Digital Waffle | LinkedIn"`).
///
/// Deliberately conservative, because a job title legitimately contains both
/// separators: `"Engineer | Payments"` and `"Full-Stack Developer"` must survive
/// untouched. That rules out "drop everything after the last separator", which is
/// why this matches against a known name instead. Only the LAST segment is
/// considered and only one is removed, so `"Dev | Digital Waffle | LinkedIn"`
/// loses `LinkedIn` and keeps the employer — which the company heuristics below
/// are what actually read.
pub(super) fn strip_site_suffix(title: &str, site: Option<&str>, host: &str) -> String {
    // The host's most distinctive label: `www.linkedin.com` -> `linkedin`.
    let host_label = host
        .rsplit('.')
        .nth(1)
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    let matches_site = |candidate: &str| {
        let c = candidate.trim();
        if c.is_empty() {
            return false;
        }
        site.map(|s| s.trim().eq_ignore_ascii_case(c))
            .unwrap_or(false)
            || (!host_label.is_empty() && c.eq_ignore_ascii_case(host_label))
    };

    for sep in [" | ", " - ", " \u{2013} ", " \u{2014} "] {
        if let Some((head, tail)) = title.rsplit_once(sep) {
            if matches_site(tail) && !head.trim().is_empty() {
                return head.trim().to_string();
            }
        }
    }
    title.trim().to_string()
}

/// A LinkedIn-shaped company signal: an `<img>` whose `alt` reads
/// `"Company logo for, <name>"` (or `"<name> logo"`). A HEURISTIC, not a
/// contract — it reads markup we do not control and is only consulted after
/// JSON-LD and `og:site_name` have both come up empty (issue #1239, where the
/// employer name was present in the captured DOM but only as incidental `alt`
/// text). Returns `None` on any page without that shape, so the caller falls
/// through to the host exactly as before.
pub(super) fn company_from_logo_alt(doc: &Html) -> Option<String> {
    let sel = Selector::parse("img[alt]").ok()?;
    for el in doc.select(&sel) {
        let alt = el.value().attr("alt")?.trim();
        // `"Company logo for, Digital Waffle"` — the comma is LinkedIn's own.
        let name = alt
            .strip_prefix("Company logo for,")
            .or_else(|| alt.strip_prefix("Company logo for"))
            .map(str::trim)
            .or_else(|| alt.strip_suffix(" logo").map(str::trim))
            .filter(|s| !s.is_empty());
        if let Some(name) = name {
            return Some(name.chars().take(GENERIC_FIELD_CAP).collect());
        }
    }
    None
}

/// Best-effort real employer name for the generic fallback. Tries JSON-LD
/// (`JobPosting.hiringOrganization.name`, incl. an `@graph` array), then
/// `og:site_name`, then the `<img alt>` logo heuristic
/// ([`company_from_logo_alt`]). Returns `None` when none is present so the
/// caller can fall back to the host.
pub(super) fn parse_generic_company(html: &str) -> Option<String> {
    let doc = Html::parse_document(html);

    if let Ok(sel) = Selector::parse(r#"script[type="application/ld+json"]"#) {
        for node in doc.select(&sel) {
            let raw = node.text().collect::<String>();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(name) = json_ld_company(&json) {
                    let name = name.trim();
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            }
        }
    }

    if let Ok(sel) = Selector::parse(r#"meta[property="og:site_name"]"#) {
        if let Some(name) = doc
            .select(&sel)
            .next()
            .and_then(|e| e.value().attr("content"))
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Some(name.chars().take(GENERIC_FIELD_CAP).collect());
        }
    }

    // Last resort before the caller falls back to the bare host.
    company_from_logo_alt(&doc)
}

/// Pull `hiringOrganization.name` from a JSON-LD value at any depth, tolerating a
/// single object, a string org, an `@graph` array, and arbitrary nesting.
// ponytail: same depth-12 cap as `find_job` — cyclic/pathological-nesting guard.
pub(super) fn json_ld_company(json: &serde_json::Value) -> Option<String> {
    fn org_name(node: &serde_json::Value, depth: u8) -> Option<String> {
        match node.get("hiringOrganization") {
            Some(serde_json::Value::String(s)) => return Some(s.clone()),
            Some(org @ serde_json::Value::Object(_)) => {
                if let Some(name) = org.get("name").and_then(|n| n.as_str()) {
                    return Some(name.to_string());
                }
            }
            _ => {}
        }
        if depth >= 12 {
            return None;
        }
        match node {
            serde_json::Value::Array(arr) => arr.iter().find_map(|n| org_name(n, depth + 1)),
            serde_json::Value::Object(map) => map.values().find_map(|n| org_name(n, depth + 1)),
            _ => None,
        }
    }
    org_name(json, 0)
}
