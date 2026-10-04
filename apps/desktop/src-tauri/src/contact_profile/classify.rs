//! Link classification: sorts the links extracted from an uploaded résumé into
//! the profile's named fields by NAME and SHAPE, never by position.

use super::{ContactLink, ContactProfile};
use crate::extraction::types::Link;
use crate::model::rich::url_label;

/// Hosts that are job boards / aggregators / employer ATS — never a personal
/// contact link, so they must not seed LinkedIn / GitHub / Website.
const JOB_BOARD_HOSTS: &[&str] = &[
    "indeed.com",
    "glassdoor.com",
    "stepstone.de",
    "stepstone.com",
    "monster.com",
    "ziprecruiter.com",
    "lever.co",
    "greenhouse.io",
    "workday.com",
    "myworkdayjobs.com",
    "ashbyhq.com",
    "smartrecruiters.com",
    "recruitee.com",
    "personio.de",
    "arbeitnow.com",
    "xing.com",
];

pub(super) fn host_of(url: &str) -> Option<String> {
    let lower = url.trim().to_lowercase();
    let no_scheme = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .unwrap_or(&lower);
    let host = no_scheme.split(['/', '?', '#']).next()?;
    Some(host.trim_start_matches("www.").to_string())
}

fn host_is(url: &str, domain: &str) -> bool {
    host_of(url).is_some_and(|h| h == domain || h.ends_with(&format!(".{domain}")))
}

/// A personal LinkedIn profile is `/in/…`. Company (`/company/…`), school
/// (`/school/…`) and job (`/jobs/…`) pages are NOT the candidate's profile — these
/// are exactly the company-link pool that used to leak into the header.
fn is_personal_linkedin(url: &str) -> bool {
    host_is(url, "linkedin.com") && url.to_lowercase().contains("/in/")
}

/// A personal Xing profile is `/profile/…` — same gate shape as
/// [`is_personal_linkedin`]'s `/in/`. `xing.com` is also a [`JOB_BOARD_HOSTS`]
/// entry (Xing hosts job listings too), so without this a legitimate DACH
/// candidate's personal profile link reads as job-board-adjacent to
/// `validate::pdf_render_issues`'s header-band warning. Mirrors
/// `isPersonalXing` in `packages/prompts/src/generate/links/links.ts`.
pub(crate) fn is_personal_xing(url: &str) -> bool {
    host_is(url, "xing.com") && url.to_lowercase().contains("/profile/")
}

/// A github.com URL. Combined with `is_profile_shaped` at the call site so only
/// `github.com/<user>` (not `/<user>/<repo>`) is promoted to the candidate's
/// GitHub — a repo link is a project reference, not an identity.
fn is_github(url: &str) -> bool {
    host_is(url, "github.com")
}

/// Known social/portfolio platform hosts whose profile page belongs on the
/// contact line. Mirrors `PROFILE_DOMAINS` in
/// `packages/prompts/src/generate/links/links.ts` — keep the two lists in
/// sync, WITH THREE DELIBERATE, NAMED EXCEPTIONS (a stale "keep in sync" claim
/// with no named exceptions is what caused half this branch's parity
/// findings — this list is not silently allowed to drift again):
/// `about.me`/`carrd.co` are TS-only here because Rust splits what TS keeps
/// as one list into two — they're covered on this side by [`WEBSITE_HOSTS`]
/// instead (the "Website" field, not `extra_links`). `xing.com` is TS-only
/// because TS's `isPersonalXing` gate (the `/profile/` shape, mirrored here by
/// [`is_personal_xing`]) is only wired into the import path on the TS side —
/// Rust's `classify_contact_links` has no Xing handling yet.
const PROFILE_HOSTS: &[&str] = &[
    "linkedin.com",
    "github.com",
    "gitlab.com",
    "twitter.com",
    "x.com",
    "behance.net",
    "dribbble.com",
    "medium.com",
    "stackoverflow.com",
    "dev.to",
    "codepen.io",
    "youtube.com",
    "youtu.be",
    "notion.so",
    "figma.com",
    "npmjs.com",
    "crates.io",
    "solo.to",
    "bio.link",
    "linktr.ee",
    "bento.me",
];

fn is_profile_host(url: &str) -> bool {
    PROFILE_HOSTS.iter().any(|d| host_is(url, d))
}

/// Non-empty path segments of `url` (host, query and fragment stripped).
/// Mirrors `pathSegments()` in links.ts (only the *count* matters here, so
/// unlike the TS version this does not percent-decode).
fn path_segments(url: &str) -> Vec<&str> {
    let trimmed = url.trim();
    let no_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let path = match no_scheme.find('/') {
        Some(idx) => no_scheme[idx..].split(['?', '#']).next().unwrap_or(""),
        None => "",
    };
    path.split('/').filter(|s| !s.is_empty()).collect()
}

/// A bare-root URL — host only, no meaningful path. The shape of a homepage.
/// Mirrors `isBareRoot()` in links.ts.
fn is_bare_root(url: &str) -> bool {
    path_segments(url).is_empty()
}

/// Is this platform URL a *profile* (belongs on the contact line) rather than a
/// deep link to a specific repo/article (a project reference, which belongs in
/// the résumé body, not the header)? `github.com/<user>` is a profile;
/// `github.com/<user>/<repo>` is a project. Mirrors `isProfileShaped()` in
/// links.ts.
fn is_profile_shaped(url: &str) -> bool {
    if host_is(url, "github.com") || host_is(url, "gitlab.com") {
        return path_segments(url).len() <= 1;
    }
    true
}

/// A link shaped like a **platform profile** — the only kind of link that may
/// seed `extra_links`: a profile-shaped host from [`PROFILE_HOSTS`] (GitHub,
/// Dribbble, Behance, …), or a personal LinkedIn (`/in/`) profile. LinkedIn
/// keeps the stricter `is_personal_linkedin` gate instead of the generic
/// `is_profile_shaped` rule — a company/school page is otherwise
/// indistinguishable by shape but must never seed the header. A bare-root
/// *personal* domain (no known platform) is deliberately excluded here — at
/// most one such domain is ever admitted to the profile at all, as `website`
/// (see the fallback in [`classify_contact_links`]); every other one is a
/// body/project link and must never re-enter the profile. Shared rule with
/// `isProfileUrl`/`isProfileShaped`/`classifyLinks` in
/// `packages/prompts/src/generate/links/links.ts`: a platform-profile URL
/// stays on the contact side; a non-platform URL is admitted at most once
/// (`Website`) — every other one is a body link.
fn is_platform_profile_link(url: &str) -> bool {
    if host_is(url, "linkedin.com") {
        return is_personal_linkedin(url);
    }
    is_profile_host(url) && is_profile_shaped(url)
}

/// Personal-site / link-in-bio hosts that belong under "Website".
const WEBSITE_HOSTS: &[&str] = &[
    "solo.to",
    "bio.link",
    "linktr.ee",
    "bento.me",
    "about.me",
    "carrd.co",
];

/// Public for `validate::pdf_render_issues` — a header-band link is still
/// worth a warning when it resolves to a known job-board/ATS host, even on the
/// text-derived-header path where the profile isn't the source of truth.
pub(crate) fn is_job_board(url: &str) -> bool {
    JOB_BOARD_HOSTS.iter().any(|d| host_is(url, d))
}

/// Classify extracted résumé links into a [`ContactProfile`] by NAME and SHAPE,
/// not by position. Picks the first personal LinkedIn (`/in/`), the first
/// profile-shaped GitHub (`github.com/<user>`, never `/<user>/<repo>`), and a
/// personal website (a known link-in-bio host, else a bare-root, non-platform
/// `http(s)` link — an apex host wins over any candidate that is one of its
/// own subdomains, then first-seen decides; this is order-independent). Every
/// remaining *platform-profile* link — a profile-shaped platform profile
/// (Dribbble, Behance, a second GitHub user) — is kept as a labelled
/// [`ContactLink`] in `extra_links`. A bare-root personal domain that was not
/// promoted to `website`, and any deep-path project/demo/article/repo link,
/// never enters the profile at all — it belongs in the résumé body, not the
/// header. Shared rule with `classifyLinks` in
/// `packages/prompts/src/generate/links/links.ts`: a platform-profile URL
/// stays on the contact side (LinkedIn gated to `/in/`, same as
/// `is_personal_linkedin` here); a non-platform URL is admitted at most once,
/// as `Website`, via the same order-independent apex-over-subdomain
/// preference (`pickWebsiteUrl` there mirrors `is_subdomain_of_another` /
/// `is_apex_of_another` here, dot-prefixed suffix check included) — every
/// other bare-root candidate is a body/project link.
///
/// This is a suggestion to seed the editable profile, never the final header on
/// its own.
pub fn classify_contact_links(links: &[Link]) -> ContactProfile {
    let mut profile = ContactProfile::default();
    for link in links {
        let url = link.url.trim();
        if url.is_empty() {
            continue;
        }
        if let Some(email) = url.strip_prefix("mailto:") {
            profile.email.get_or_insert_with(|| email.to_string());
            continue;
        }
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            continue;
        }
        if profile.linkedin.is_none() && is_personal_linkedin(url) {
            profile.linkedin = Some(url.to_string());
            continue;
        }
        if profile.github.is_none() && is_github(url) && is_profile_shaped(url) {
            profile.github = Some(url.to_string());
            continue;
        }
        if profile.website.is_none() && WEBSITE_HOSTS.iter().any(|d| host_is(url, d)) {
            profile.website = Some(url.to_string());
            continue;
        }
    }
    // Website fallback: a non-job-board, non-platform, bare-root http(s) link,
    // so a personal portfolio homepage is still surfaced — but an
    // employer/company URL or a deep link (e.g. a project demo path) never is.
    // `!is_profile_host` subsumes the old explicit linkedin/github checks.
    //
    // Which candidate wins is order-independent: an apex host (one that is
    // itself the parent of another candidate host in this same document, e.g.
    // `apex.dev` alongside `sub.apex.dev`) is preferred over every standalone
    // candidate, because the apex/subdomain relationship is stronger, shape-
    // based evidence of "the" personal domain than raw document position.
    // Among hosts tied on that signal, first-seen decides.
    if profile.website.is_none() {
        let candidates: Vec<(String, &str)> = links
            .iter()
            .filter_map(|link| {
                let url = link.url.trim();
                let is_candidate = (url.starts_with("http://") || url.starts_with("https://"))
                    && !is_job_board(url)
                    && !is_profile_host(url)
                    && is_bare_root(url);
                if !is_candidate {
                    return None;
                }
                host_of(url).map(|h| (h, url))
            })
            .collect();
        let hosts: Vec<&str> = candidates.iter().map(|(h, _)| h.as_str()).collect();
        let is_subdomain_of_another = |host: &str| {
            hosts
                .iter()
                .any(|o| *o != host && host.ends_with(&format!(".{o}")))
        };
        let is_apex_of_another = |host: &str| {
            hosts
                .iter()
                .any(|o| *o != host && o.ends_with(&format!(".{host}")))
        };
        let apex_pick = candidates
            .iter()
            .find(|(h, _)| !is_subdomain_of_another(h) && is_apex_of_another(h));
        let first_pick = candidates.iter().find(|(h, _)| !is_subdomain_of_another(h));
        if let Some((_, url)) = apex_pick.or(first_pick) {
            profile.website = Some(url.to_string());
        }
    }
    // Extras: every other platform-profile http(s) link, labelled by domain
    // (Dribbble, Behance, a second GitHub user, …). A bare-root personal
    // domain that lost the `website` slot above is NOT an extra — it never
    // re-enters the profile. Skips job boards and the links already promoted
    // to a named field, and de-dupes by URL so the same link is never listed
    // twice.
    let named: std::collections::BTreeSet<&str> = [
        profile.linkedin.as_deref(),
        profile.github.as_deref(),
        profile.website.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect();
    for link in links {
        let url = link.url.trim();
        if !(url.starts_with("http://") || url.starts_with("https://"))
            || is_job_board(url)
            || !is_platform_profile_link(url)
            || named.contains(url)
            || profile.extra_links.iter().any(|e| e.url == url)
        {
            continue;
        }
        profile.extra_links.push(ContactLink {
            label: url_label(url),
            url: url.to_string(),
        });
    }
    profile
}

#[cfg(test)]
mod tests;
