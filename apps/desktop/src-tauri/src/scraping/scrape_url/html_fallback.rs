//! Generic HTML fallback: whole-document `<title>`/meta-description parse,
//! the extension's `[data-ajh-job-root]` hint, JSON-LD `JobPosting`, and
//! Next.js `__NEXT_DATA__` — in that precedence order (see [`parse_from_html`]).

use std::collections::HashMap;

use scraper::{Html, Selector};

use crate::scraping::types::JobPosting;

use super::generic::{
    main_content_text, og_site_name, parse_generic_company, parse_generic_html,
    readability_content_text, strip_site_suffix, GENERIC_FIELD_CAP,
};

// ── Generic HTML fallback ───────────────────────────────────────────────────

/// Build a `JobPosting` from already-fetched HTML.  The extension-bridge
/// **Scan mode** supplies the authenticated DOM (a logged-in board page the
/// desktop's anonymous fetch can't see), so it reuses this exact parse path
/// instead of re-fetching.  [`resolve`] calls this after it has followed any
/// redirect chain and exhausted the named-board re-dispatch.
///
/// Prefers JSON-LD `JobPosting` fields (title/description/location/company) when
/// the page ships them — structured data always wins over any DOM guess. Below
/// that, the base pass is the generic `<title>`/`<h1>` + meta description parse,
/// with the extension's `[data-ajh-job-root="true"]` hint (when present)
/// overriding title/description FIELD BY FIELD rather than wholesale — see the
/// per-field merge note below. `parse_generic_company` supplies the employer
/// name. Always returns `Some` for a successfully-parsed document — the title
/// may be an empty string (e.g. a page with only a meta description) so the
/// description-on-demand flow still surfaces the page. (The fetch half
/// short-circuits earlier on a non-success status / rejected host.)
pub fn parse_from_html(url: &str, html: &str) -> Option<JobPosting> {
    // Base: whole-document <title>/first-<h1> + meta description (today's floor).
    let (mut title, mut description) = parse_generic_html(html);

    // Prefer the extension's `[data-ajh-job-root="true"]` hint (Scan-mode's
    // best-effort "main job content" mark — see `markLikelyJobNode` in
    // `apps/extension/src/content.ts`) over the base pass above, but FIELD BY
    // FIELD rather than wholesale: override `title` only when the hinted
    // subtree found a non-empty one, override `description` only when it found
    // one. A wholesale swap would be wrong on a thin-body page (e.g. an ATS
    // embeds the real description in an iframe and the hinted node is just an
    // `<h1>`) — it would clobber a good document-level meta description with
    // nothing useful. A mis-marked/hostile hint (wrong element, script/
    // whitespace-only) yields (empty, `None`) for both fields, so it overrides
    // neither — the per-field fallback is the guarantee that a bad hint can
    // never make results worse than before.
    //
    // `hint_title_used` tracks whether the hint actually supplied a usable
    // title — the signal the last-resort fallback below uses to tell "thin
    // hint" (real signal, just no body) apart from "hostile/mis-marked hint"
    // (no signal at all, treat as if there were no hint).
    let mut hint_title_used = false;
    if let Some((hint_title, hint_description)) = job_root_generic_html(html) {
        if !hint_title.is_empty() {
            title = hint_title;
            hint_title_used = true;
        }
        if hint_description.is_some() {
            description = hint_description;
        }
    }
    let mut location = None;

    // JSON-LD `JobPosting` is the richest source when present — let it override
    // the generic title/description and supply a location the meta tags lack.
    if let Some(jl) = json_ld_job_posting(html) {
        if !jl.title.is_empty() {
            title = jl.title;
        }
        if jl.description.is_some() {
            description = jl.description;
        }
        if jl.location.is_some() {
            location = jl.location;
        }
    }

    // `__NEXT_DATA__` fallback: fill ONLY fields JSON-LD left missing (don't
    // clobber good JSON-LD values).
    if title.is_empty() || description.is_none() {
        if let Some(nd) = next_data_job(html) {
            if title.is_empty() && !nd.title.is_empty() {
                title = nd.title;
            }
            if description.is_none() {
                description = nd.description;
            }
            if location.is_none() {
                location = nd.location;
            }
        }
    }

    // Whole-document last-resort description — SKIPPED when the hint already
    // supplied a real title. `job_root_generic_html` scopes its own
    // description search to that same hinted subtree, so if it honestly found
    // no body text there (a title-only hint — e.g. the real description
    // renders client-side in an ATS iframe the outerHTML capture can't see),
    // escalating to a whole-DOCUMENT guess risks landing on an unrelated block
    // (e.g. a bigger related-jobs sidebar) instead of just admitting there's
    // no description to show. A hint that yielded NOTHING (no title either —
    // hostile/mis-marked) carries no signal, so the whole-document heuristic
    // chain still runs exactly as if there were no hint at all.
    //
    // Within that chain, `readability_content_text` (a real Mozilla-Readability
    // port) runs FIRST — it scores/prunes nav, footer, and boilerplate rather
    // than just picking the largest `main`/`article` block, so it's the
    // higher-precision guess on a page with no JSON-LD. `main_content_text`'s
    // largest-block guess stays as the final fallback for when readability's
    // own pre-check (`is_probably_readable`) or `parse()` comes back empty.
    if description.is_none() && !hint_title_used {
        description = readability_content_text(url, html).or_else(|| main_content_text(html));
    }

    let host = reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();
    // Drop a site-branding suffix the generic <title> carries (#1239). Done
    // here rather than in `parse_generic_html` because it needs the host, and
    // only AFTER the hint/JSON-LD passes above have had their say — a title
    // they supplied is already clean.
    let site_name = og_site_name(html);
    title = strip_site_suffix(&title, site_name.as_deref(), &host);
    title = title.chars().take(GENERIC_FIELD_CAP).collect();

    // Prefer a real employer name (JSON-LD / og:site_name / logo alt), then the
    // ATS board slug (a React-rendered `job-boards.greenhouse.io/<co>/…` page
    // carries neither; #1359), over the bare host.
    let company = parse_generic_company(html)
        .or_else(|| {
            let slug = crate::scraping::ats_ref::extract_ats_ref(url)?.slug;
            let decoded = urlencoding::decode(&slug).map_or(slug.clone(), |d| d.into_owned());
            Some(
                decoded
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(GENERIC_FIELD_CAP)
                    .collect(),
            )
        })
        .unwrap_or(host);

    Some(JobPosting {
        id: format!("url:{}", url),
        external_id: None,
        title,
        company,
        location,
        url: url.to_string(),
        source: "url".to_string(),
        description,
        requirements: None,
        posted_at: None,
        captured_at: chrono::Utc::now().timestamp_millis(),
        extra: HashMap::new(),
    })
}

/// Extraction from the extension's best-effort `[data-ajh-job-root="true"]`
/// hint (pinned to the contract value — `markLikelyJobNode` in
/// `apps/extension/src/content.ts` only ever sets `"true"`). Title is the first
/// `h1` inside the hinted subtree; description is the subtree's own rendered
/// text, EXCLUDING that title heading's own markup (see the `JOB_ROOT_TITLE_RE`
/// strip below) so a hinted node that is just a heading — the real body
/// rendered elsewhere, e.g. an ATS iframe the outerHTML capture can't see —
/// yields no description at all rather than a title-redundant stub. Scoping to
/// the hinted node — rather than re-running the largest-`main`/`article`-block
/// guess ([`main_content_text`]) over the whole document — lets a page with
/// several `main`/`article`-shaped blocks (e.g. a related-jobs sidebar bigger
/// than the actual posting) resolve to the right one. Returns `None` when no
/// hinted node exists in the document at all, so the no-hint path (including
/// every server-fetch resolve where the hint can never be present) is
/// untouched. The caller (`parse_from_html`) applies title and description
/// independently, so either field alone may come back empty/`None`.
///
/// `pub(crate)` so `extension_bridge::import_flow`'s canonical (SPA/list-view)
/// branch can call this HINT-SCOPED extraction directly on the extension's
/// captured HTML, instead of the whole-document `parse_from_html` — a list
/// shell (e.g. LinkedIn search results) commonly carries its own SEO
/// `JobPosting` JSON-LD for an UNRELATED job, and `parse_from_html`'s
/// precedence lets JSON-LD override the hint, which would silently import the
/// wrong job. Scoping to just this extraction sidesteps that entirely.
// ponytail: single-node lookup + a couple of child selectors, no depth cap
// needed (unlike the JSON-LD/NEXT_DATA walks) — scoped to whatever the
// extension already marked.
pub(crate) fn job_root_generic_html(html: &str) -> Option<(String, Option<String>)> {
    // Cheap substring check before the full-document reparse below: every
    // server-fetch resolve call hits this with no hint attribute present at
    // all, so skip `Html::parse_document` entirely on that (overwhelmingly
    // common) path.
    if !html.contains("data-ajh-job-root") {
        return None;
    }

    let doc = Html::parse_document(html);
    let root_sel = Selector::parse(r#"[data-ajh-job-root="true"]"#).ok()?;
    // First-in-DOM-order match is the intended tie-break if a page ever
    // contains several hinted nodes.
    let root = doc.select(&root_sel).next()?;

    // Unlike this crate's own `html_to_text`, `html_to_markdown`'s `htmd` backend
    // does not treat `<script>`/`<style>` as non-rendered — it leaks their raw
    // contents into the output. A hostile/mis-marked hint node containing only a
    // tracking script must not read as a "real" title or description, so strip
    // them first — before extracting EITHER field, not just the description.
    let inner_html = root.inner_html();
    let cleaned = JOB_ROOT_SCRIPT_STYLE_RE.replace_all(&inner_html, " ");
    let cleaned_doc = Html::parse_fragment(&cleaned);

    let title_sel = Selector::parse("h1").ok()?;
    let title = cleaned_doc
        .select(&title_sel)
        .next()
        .map(|e| e.text().collect::<String>().trim().to_string())
        .unwrap_or_default();

    // Exclude ONLY the title heading (the first <h1>) from the description
    // source: an h1-only hinted subtree must not turn its own title into a
    // redundant description stub that would then clobber a real document-level
    // meta description in the caller's per-field merge. `replacen(.., 1, ..)`
    // rather than `replace_all` — a later `<h1>` is a legitimate section
    // heading (e.g. "Responsibilities") and must survive into the description.
    let body_html = JOB_ROOT_TITLE_RE.replacen(&cleaned, 1, " ");
    let description = crate::scraping::http::html_to_markdown(&body_html);
    let description = (!description.trim().is_empty()).then_some(description);

    Some((title, description))
}

pub(super) static JOB_ROOT_SCRIPT_STYLE_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?is)<(script|style)[\s\S]*?</(script|style)>").unwrap()
    });

/// Matches an `<h1>` element in the hinted subtree's (already script/style-
/// cleaned) HTML; the caller only ever strips the FIRST match (`replacen`, not
/// `replace_all`) — see the "exclude the title" note on [`job_root_generic_html`]
/// above. A later `<h1>` is a legitimate section heading, not the title.
static JOB_ROOT_TITLE_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?is)<h1[\s\S]*?</h1>").unwrap());

/// The subset of JSON-LD `JobPosting` fields the generic parse path consumes.
struct JsonLdJob {
    title: String,
    description: Option<String>,
    location: Option<String>,
}

/// A JSON-LD / `__NEXT_DATA__` description whose generator double-escaped its newlines
/// decodes to a STANDALONE literal backslash-n between tags or whitespace (a real
/// Lever/Spotify posting: backslash-n between `</p>` and `<p>`), which markdown conversion
/// then shows verbatim. Restored to a newline here, where the artefact originates, rather
/// than in the shared converter. Left alone: a token like `C:\new`, a doubled
/// backslash-backslash-n, and any text that
/// carries `<code>`/`<pre>` (there a backslash-n is the content).
pub(super) fn unescape_literal_newlines(s: &str) -> std::borrow::Cow<'_, str> {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"(^|[\s>])\\n($|[\s<])").unwrap());
    if s.contains("<code") || s.contains("<pre") {
        return s.into();
    }
    RE.replace_all(s, "$1\n$2")
}

/// Format one JSON-LD `PostalAddress`-shaped node to a display string.
/// Locality-first (`"City, Region"` / `"City"` / `"Region"`); `addressCountry`
/// is a fallback ONLY when both locality and region are absent.
// ponytail: country used only as a fallback when no locality/region, to preserve
// the locality-first golden ("Berlin, BE" stays "Berlin, BE", not ", BE, DE").
fn fmt_address(addr: &serde_json::Value) -> Option<String> {
    let locality = addr.get("addressLocality").and_then(|s| s.as_str());
    let region = addr.get("addressRegion").and_then(|s| s.as_str());
    match (locality, region) {
        (Some(c), Some(r)) => Some(format!("{c}, {r}")),
        (Some(c), None) => Some(c.to_string()),
        (None, Some(r)) => Some(r.to_string()),
        (None, None) => addr
            .get("addressCountry")
            .and_then(|s| s.as_str())
            .map(str::to_string),
    }
}

/// Pull a display location from a `JobPosting`'s `jobLocation`, which may be a
/// single node OR an array of nodes. Each node's `address` is formatted via
/// [`fmt_address`]; multiple addresses join with `"; "`.
fn job_location(node: &serde_json::Value) -> Option<String> {
    let parts: Vec<String> = match node.get("jobLocation") {
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|n| n.get("address").and_then(fmt_address))
            .collect(),
        Some(single) => single
            .get("address")
            .and_then(fmt_address)
            .into_iter()
            .collect(),
        None => return None,
    };
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("; "))
    }
}

/// Does this node's `@type` denote a `JobPosting`? Tolerates a string or an
/// array of types (schema.org allows multiple types on one node).
fn is_job_posting(node: &serde_json::Value) -> bool {
    match node.get("@type") {
        Some(serde_json::Value::String(s)) => s == "JobPosting",
        Some(serde_json::Value::Array(arr)) => arr.iter().any(|v| v.as_str() == Some("JobPosting")),
        _ => false,
    }
}

/// Extract a [`JsonLdJob`] from a single node IF it is a titled `JobPosting`.
fn job_from_node(node: &serde_json::Value) -> Option<JsonLdJob> {
    if !is_job_posting(node) {
        return None;
    }
    let title = node
        .get("title")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if title.is_empty() {
        return None;
    }
    let description = node
        .get("description")
        .and_then(|s| s.as_str())
        .map(|d| crate::scraping::http::html_to_markdown(&unescape_literal_newlines(d)))
        .filter(|s| !s.trim().is_empty());
    Some(JsonLdJob {
        title,
        description,
        location: job_location(node),
    })
}

/// Walk a JSON-LD value (object values, array elements, `@graph` — which is just
/// an object-valued array the recursion descends naturally) for the FIRST titled
/// `JobPosting` node.
// ponytail: depth-12 cap guards cyclic/pathologically-nested JSON-LD; raise it if
// a real page legitimately nests a JobPosting deeper.
fn find_job(node: &serde_json::Value, depth: u8) -> Option<JsonLdJob> {
    if let Some(job) = job_from_node(node) {
        return Some(job);
    }
    if depth >= 12 {
        return None;
    }
    match node {
        serde_json::Value::Array(arr) => arr.iter().find_map(|n| find_job(n, depth + 1)),
        serde_json::Value::Object(map) => map.values().find_map(|n| find_job(n, depth + 1)),
        _ => None,
    }
}

/// Best-effort pull of a JSON-LD `JobPosting` node from anywhere in any
/// `application/ld+json` block (top-level, `@graph`, or arbitrarily nested) —
/// title/description/jobLocation. Returns `None` when no `JobPosting` node
/// carries a usable title.
fn json_ld_job_posting(html: &str) -> Option<JsonLdJob> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(r#"script[type="application/ld+json"]"#).ok()?;
    for node in doc.select(&sel) {
        let raw = node.text().collect::<String>();
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        if let Some(job) = find_job(&json, 0) {
            return Some(job);
        }
    }
    None
}

/// Whether the page ships a titled JSON-LD `JobPosting` of its own.
pub(super) fn has_json_ld_job_posting(html: &str) -> bool {
    json_ld_job_posting(html).is_some()
}

/// `__NEXT_DATA__` fallback: Next.js ships the page's props as JSON in a
/// `script#__NEXT_DATA__` blob. Find a `JobPosting` node OR, failing that, a
/// job-shaped node (a non-empty string `title` plus at least one of
/// `description` / `hiringOrganization` / `jobLocation`) and pull the same
/// fields the JSON-LD path does.
// ponytail: Next.js prop-shape sniffing; upgrade path = a per-board JSON path if a
// real page needs one. Same depth-12 cap as `find_job`.
fn next_data_job(html: &str) -> Option<JsonLdJob> {
    /// Pull a `JsonLdJob` out of a job-shaped node (already known to have a title).
    fn from_jobish(node: &serde_json::Value) -> JsonLdJob {
        let title = node
            .get("title")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let description = node
            .get("description")
            .and_then(|s| s.as_str())
            .map(|d| crate::scraping::http::html_to_markdown(&unescape_literal_newlines(d)))
            .filter(|s| !s.trim().is_empty());
        JsonLdJob {
            title,
            description,
            location: job_location(node),
        }
    }
    fn is_jobish(node: &serde_json::Value) -> bool {
        let has_title = node
            .get("title")
            .and_then(|s| s.as_str())
            .is_some_and(|s| !s.trim().is_empty());
        has_title
            && (node.get("description").is_some()
                || node.get("hiringOrganization").is_some()
                || node.get("jobLocation").is_some())
    }
    fn find(node: &serde_json::Value, depth: u8) -> Option<JsonLdJob> {
        // Prefer a real JobPosting; else accept a job-shaped node.
        if let Some(job) = job_from_node(node) {
            return Some(job);
        }
        if is_jobish(node) {
            return Some(from_jobish(node));
        }
        if depth >= 12 {
            return None;
        }
        match node {
            serde_json::Value::Array(arr) => arr.iter().find_map(|n| find(n, depth + 1)),
            serde_json::Value::Object(map) => map.values().find_map(|n| find(n, depth + 1)),
            _ => None,
        }
    }

    let doc = Html::parse_document(html);
    let sel = Selector::parse("script#__NEXT_DATA__").ok()?;
    let raw = doc.select(&sel).next()?.text().collect::<String>();
    let json = serde_json::from_str::<serde_json::Value>(&raw).ok()?;
    find(&json, 0)
}
