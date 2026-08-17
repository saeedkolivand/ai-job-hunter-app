use scraper::{Html, Selector};
use serde_json::Value;

use super::ProfileData;
use crate::error::{AppError, AppResult};

/// Fetch a LinkedIn public profile page and extract structured data.
///
/// LinkedIn embeds `application/ld+json` blocks with schema.org Person/Organization
/// data on public profile pages. We parse those first; visible HTML sections serve
/// as a fallback for experience, education, and skills.
pub async fn import(url: &str) -> AppResult<ProfileData> {
    let html = fetch_page(url).await?;
    parse_profile(&html)
}

/// Parse a fetched LinkedIn page into [`ProfileData`]. Pure (no network), so
/// the parse-strictness gate below is unit-testable without a fetch mock.
fn parse_profile(html: &str) -> AppResult<ProfileData> {
    let document = Html::parse_document(html);

    let ld = extract_ld_json(&document);

    // The `ld+json` Person block is LinkedIn's own signal that this response is
    // a real, logged-out-visible profile page. An authwall / login redirect /
    // authenticated SPA shell never carries one — but it DOES still carry a
    // `<title>`/`og:title` tag (e.g. "LinkedIn Login, Sign in..."), which is why
    // falling back to `og:title` for `name` used to make an authwall parse as a
    // "successful" import of an almost-empty resume. Gate on the JSON-LD name
    // before trusting anything else on the page.
    let ld_name = ld
        .as_ref()
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .map(clean)
        .filter(|n| !n.is_empty());
    let Some(name) = ld_name else {
        log::warn!("[profile_import] linkedin parse failed: no ld+json Person block");
        return Err(AppError::Parse(
            "linkedin did not return public profile data for this url — it may be private, unavailable, or the request was blocked".to_string(),
        ));
    };

    let headline = ld
        .as_ref()
        .and_then(|v| v.get("jobTitle"))
        .and_then(|v| v.as_str())
        .map(clean)
        .or_else(|| extract_meta(&document, "og:title").map(|t| strip_linkedin_suffix(&t)));
    let summary = ld
        .as_ref()
        .and_then(|v| v.get("description"))
        .and_then(|v| v.as_str())
        .map(clean);
    let location = ld
        .as_ref()
        .and_then(|v| v.get("address"))
        .and_then(|v| v.get("addressLocality"))
        .and_then(|v| v.as_str())
        .map(clean);

    // Extract lists from HTML structure (visible to crawlers)
    let experience = extract_list_section(&document, "experience");
    let education = extract_list_section(&document, "education");
    let skills = extract_skills(&document, &ld);

    Ok(ProfileData {
        name: Some(name),
        headline,
        summary,
        experience,
        education,
        skills,
        location,
        platform: "linkedin".to_string(),
    })
}

// ── HTTP ─────────────────────────────────────────────────────────────────────

async fn fetch_page(url: &str) -> AppResult<String> {
    // Deliberately anonymous — this is NOT an oversight. LinkedIn only embeds
    // the `ld+json` Person block this parser depends on in the response served
    // to logged-out/crawler requests. Once the user connects LinkedIn (Settings
    // → Accounts) and `li_at` would be attached, the same URL instead returns
    // the authenticated SPA shell / authwall, which has no `ld+json` to parse —
    // attaching the session cookie jar here would break the only case that
    // currently works. Board scraping and the login flow still use
    // `board_login::build_authed_client` elsewhere; this file just never does.
    let has_session = has_linkedin_session();

    // `url` is user-pasted / attacker-influenced input (the classic SSRF
    // surface), so it must never reach a plain client — route through
    // `get_guarded_following_redirects` (net/http.rs), this repo's SSRF
    // control: it IP-validates the host (rejecting a private/loopback/
    // cloud-metadata literal or a DNS-rebinding target) before connecting, and
    // re-validates every redirect hop the same way. Plain `get_guarded`
    // (single-hop, `Policy::none()`) is not enough here — LinkedIn 301s the
    // bare apex (`linkedin.com/in/x`, which `detect_platform` accepts) to
    // `www.linkedin.com/in/x`, so a no-redirect fetch would die on the very
    // first hop for a large share of legitimately pasted URLs. The small hop
    // budget mirrors `scraping::scrape_url::resolve_uncached`'s reasoning.
    let resp = crate::net::http::get_guarded_following_redirects(url, 3)
        .await
        .map_err(|e| {
            // Log the error's stable category, not its message: an `AppError`
            // built from a `reqwest::Error` (`AppError::from`) does not strip
            // the request URL the way the old `.without_url()` call did, and
            // the profile slug in it is PII.
            log::warn!(
                "[profile_import] linkedin fetch failed: has_session={has_session} error_code={}",
                e.code()
            );
            AppError::Network("could not reach linkedin".to_string())
        })?;

    let status = resp.status();
    if !status.is_success() {
        log::warn!(
            "[profile_import] linkedin fetch non-2xx: status={} has_session={has_session}",
            status.as_u16()
        );
        return Err(map_status(status.as_u16()));
    }

    // Bounded read: an anonymous fetch of an attacker-influenced profile URL
    // must not let a hostile/misconfigured host buffer an unbounded body into
    // memory. `read_text_capped`'s own network-error branch already strips the
    // URL (`.without_url()`), so the profile slug never reaches this log line.
    crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
        .await
        .map_err(|e| {
            log::warn!(
                "[profile_import] linkedin body read failed: has_session={has_session} error={e}"
            );
            AppError::Network("could not read the linkedin response".to_string())
        })
}

/// Map a non-2xx LinkedIn response status to the appropriate [`AppError`]. Pure
/// helper so the honest error-mapping that comes with fetching anonymously (no
/// more "log in first" advice — see [`fetch_page`]) is unit-testable without a
/// network call.
fn map_status(status: u16) -> AppError {
    if status == 429 {
        AppError::RateLimited(
            "linkedin rate-limited this request — wait a few minutes and try again".to_string(),
        )
    } else {
        AppError::Provider(
            "linkedin did not return this profile — it may be unavailable, or the request was blocked".to_string(),
        )
    }
}

/// Whether a LinkedIn session cookie jar is present on disk — diagnostic only.
/// [`fetch_page`] never attaches it; this just lets the log confirm/refute a
/// correlation between "connected" and a failed import.
fn has_linkedin_session() -> bool {
    let data_dir = crate::platform::config::data_dir();
    !crate::scraping::board_login::load_cookies(&data_dir, "linkedin").is_empty()
}

// ── JSON-LD ───────────────────────────────────────────────────────────────────

fn extract_ld_json(doc: &Html) -> Option<Value> {
    let sel = Selector::parse(r#"script[type="application/ld+json"]"#).ok()?;
    for el in doc.select(&sel) {
        let raw = el.text().collect::<String>();
        if let Ok(v) = serde_json::from_str::<Value>(&raw) {
            let type_field = v.get("@type").and_then(|t| t.as_str()).unwrap_or("");
            if type_field == "Person" {
                return Some(v);
            }
        }
    }
    None
}

// ── Meta tags ─────────────────────────────────────────────────────────────────

fn extract_meta(doc: &Html, property: &str) -> Option<String> {
    let sel = Selector::parse(&format!(r#"meta[property="{property}"]"#)).ok()?;
    doc.select(&sel)
        .next()
        .and_then(|el| el.value().attr("content"))
        .map(clean)
}

// ── HTML section extraction ───────────────────────────────────────────────────

/// LinkedIn public pages render section ids like `experience-section` / `education-section`.
/// We grab all text inside the matching section element.
fn extract_list_section(doc: &Html, section_id: &str) -> Vec<String> {
    let candidates = [
        format!("#{section_id}-section"),
        format!("section#{section_id}"),
        format!("[id*={section_id}]"),
        format!("[data-section={section_id}]"),
    ];

    for sel_str in &candidates {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(section) = doc.select(&sel).next() {
                return section
                    .text()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .chunks(3)
                    .map(|c| c.join(" · "))
                    .filter(|s| !s.is_empty())
                    .take(20)
                    .collect();
            }
        }
    }

    Vec::new()
}

fn extract_skills(doc: &Html, ld: &Option<Value>) -> Vec<String> {
    // Try JSON-LD knowsAbout array first
    if let Some(v) = ld {
        if let Some(arr) = v.get("knowsAbout").and_then(|a| a.as_array()) {
            let skills: Vec<String> = arr.iter().filter_map(|s| s.as_str()).map(clean).collect();
            if !skills.is_empty() {
                return skills;
            }
        }
    }

    // Fallback: skills section in HTML
    extract_list_section(doc, "skills")
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_linkedin_suffix(s: &str) -> String {
    s.split('|')
        .next()
        .unwrap_or(s)
        .split('-')
        .next()
        .unwrap_or(s)
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_profile: the strictness gate ────────────────────────────────────

    fn public_profile_html(name: &str) -> String {
        format!(
            r#"<html><head>
                <script type="application/ld+json">
                {{
                    "@type": "Person",
                    "name": "{name}",
                    "jobTitle": "Senior Engineer at Acme",
                    "description": "Building great software.",
                    "address": {{ "addressLocality": "Berlin, Germany" }},
                    "knowsAbout": ["Rust", "TypeScript"]
                }}
                </script>
            </head><body></body></html>"#
        )
    }

    #[test]
    fn parses_a_real_public_profile() {
        let profile = parse_profile(&public_profile_html("Jane Doe")).unwrap();
        assert_eq!(profile.name.as_deref(), Some("Jane Doe"));
        assert_eq!(profile.headline.as_deref(), Some("Senior Engineer at Acme"));
        assert_eq!(profile.location.as_deref(), Some("Berlin, Germany"));
        assert_eq!(profile.skills, vec!["Rust", "TypeScript"]);
    }

    #[test]
    fn rejects_authwall_page_with_no_ld_json() {
        // No `ld+json` Person block at all — only an og:title, the way LinkedIn's
        // login/authwall page looks. Must NOT fall back to og:title for `name`
        // (that was the "successful import of nothing" regression).
        let html = r#"<html><head>
            <meta property="og:title" content="LinkedIn Login, Sign in | LinkedIn">
        </head><body></body></html>"#;
        let err = parse_profile(html).unwrap_err();
        assert!(
            matches!(err, AppError::Parse(_)),
            "expected Parse, got {err:?}"
        );
    }

    #[test]
    fn rejects_ld_json_with_empty_name() {
        // Person block present but `name` is an empty string — must not pass the
        // gate just because the field exists.
        let html = r#"<html><head>
            <script type="application/ld+json">
            { "@type": "Person", "name": "" }
            </script>
        </head><body></body></html>"#;
        let err = parse_profile(html).unwrap_err();
        assert!(
            matches!(err, AppError::Parse(_)),
            "expected Parse, got {err:?}"
        );
    }

    #[test]
    fn falls_back_to_og_title_for_headline_only() {
        // Headline fallback to og:title is still fine — only `name` is gated on
        // ld+json.
        let html = r#"<html><head>
            <script type="application/ld+json">
            { "@type": "Person", "name": "Jane Doe" }
            </script>
            <meta property="og:title" content="Jane Doe - Senior Engineer | LinkedIn">
        </head><body></body></html>"#;
        let profile = parse_profile(html).unwrap();
        assert_eq!(profile.name.as_deref(), Some("Jane Doe"));
        assert_eq!(profile.headline.as_deref(), Some("Jane Doe"));
    }

    // ── map_status: the honest-error-mapping that comes with fetching anon ────

    #[test]
    fn map_status_429_is_rate_limited() {
        assert!(matches!(map_status(429), AppError::RateLimited(_)));
    }

    #[test]
    fn map_status_other_non_2xx_is_provider_not_auth_advice() {
        for status in [403, 404, 500, 999] {
            let err = map_status(status);
            assert!(
                matches!(err, AppError::Provider(_)),
                "status {status} should map to Provider, got {err:?}"
            );
            // Regression guard: the message must never tell the user to log in —
            // we fetch anonymously on purpose, so that advice would be actively
            // wrong (see `fetch_page`).
            let msg = err.to_string().to_lowercase();
            assert!(
                !msg.contains("log in") && !msg.contains("sign in"),
                "status {status} message must not suggest logging in: {msg:?}"
            );
        }
    }

    // `fetch_page`'s SSRF-guard test lives in a sibling `_test.rs` file, not
    // here — see the `#[path]` module declaration below for why.
}

// `fetch_page_never_dials_the_loopback_literal_it_rejects` must scope
// `AJH_DATA_DIR` (fetch_page's real-disk `has_linkedin_session` read) for its
// duration to be hermetic. That literal is banned from any non-`platform/**`
// source by `tests/architecture.rs::r4_env_access_only_in_platform` UNLESS
// the file is itself recognized as a test file (`*test.rs`/`*tests.rs`) —
// this flat `linkedin.rs` isn't, by name, so the mutation is split into its
// own `#[path]`-declared file, exactly like `commands/ai_provider`'s
// `anthropic_tests.rs`.
#[cfg(test)]
#[path = "linkedin_ssrf_test.rs"]
mod ssrf_hermetic_test;
