//! Verified posting availability (#1444), kept apart from the match score: a high-scoring
//! recommendation can be closed. [`classify`] is the pure decision; [`check_url`] is its one
//! I/O caller, fetching through the SSRF-guarded net layer, rate limited per host.
//!
//! Only an affirmative signal ever yields `Closed`/`Removed`. A login wall, captcha, 403/429,
//! timeout or any network failure is `Unknown` — never closed.

use serde::{Deserialize, Serialize};

/// Whether a posting is still taking applications, as last verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Open,
    Closed,
    Removed,
    Unknown,
}

impl Availability {
    /// A verified-dead posting: kept in storage, never offered as an actionable recommendation.
    pub fn is_dead(self) -> bool {
        matches!(self, Self::Closed | Self::Removed)
    }
}

/// Sanitized evidence category behind an [`Availability`] — never page text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityEvidence {
    Ok,
    HttpGone,
    ClosedMarker,
    Blocked,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailabilityVerdict {
    pub availability: Availability,
    #[serde(rename = "source")]
    pub evidence: AvailabilityEvidence,
}

const fn verdict(
    availability: Availability,
    evidence: AvailabilityEvidence,
) -> AvailabilityVerdict {
    AvailabilityVerdict {
        availability,
        evidence,
    }
}

/// Phrases a board shows on a posting that no longer takes applications (matched lowercase).
const CLOSED_MARKERS: &[&str] = &[
    "no longer accepting applications",
    "no longer accepting applicants",
    "this job is no longer available",
    "this job has expired",
    "this position has been filled",
    "this position is no longer",
    "this vacancy has closed",
    "job posting has expired",
    "stelle ist nicht mehr verfügbar",
    "stelle ist nicht mehr verfuegbar",
    "stelle wurde bereits besetzt",
    "diese stelle ist nicht mehr",
    "cette offre n'est plus disponible",
    "cette offre est expirée",
    "esta oferta ya no está disponible",
    "questa offerta non è più disponibile",
    "deze vacature is niet meer beschikbaar",
];

/// Interstitials that mean we never saw the posting itself.
const BLOCKED_MARKERS: &[&str] = &[
    "verify you are human",
    "unusual traffic",
    "/authwall",
    "checking your browser",
    "enable javascript and cookies to continue",
    "please complete the security check",
    "sign in to view this job",
];

/// Lowercased `html` with every `<script>`/`<style>`/`<template>` element removed, so a
/// closure string sitting unused in a translation table or JSON blob is never read as page text.
fn visible_lowercase(html: &str) -> String {
    let mut out = html.to_lowercase();
    for tag in ["script", "style", "template"] {
        let (open, close) = (format!("<{tag}"), format!("</{tag}"));
        while let Some(start) = out.find(&open) {
            let end = out[start..]
                .find(&close)
                .and_then(|c| out[start + c..].find('>').map(|g| start + c + g + 1))
                .unwrap_or(out.len());
            out.replace_range(start..end, " ");
        }
    }
    out
}

/// Pure verdict from an HTTP status (`None` = no response: timeout / network error) and a body
/// that was read to its end.
pub fn classify(status: Option<u16>, body: &str) -> AvailabilityVerdict {
    classify_read(status, body, true)
}

/// As [`classify`], but `complete = false` marks a body cut off at the size cap. A closed or
/// blocked marker found in the prefix still counts (an affirmative signal), but the absence of
/// one proves nothing about the unread rest, so it is `Unknown`, never `Open`.
pub fn classify_read(status: Option<u16>, body: &str, complete: bool) -> AvailabilityVerdict {
    use Availability::{Closed, Open, Removed, Unknown};
    use AvailabilityEvidence as E;
    match status {
        None => verdict(Unknown, E::Error),
        Some(404 | 410) => verdict(Removed, E::HttpGone),
        Some(401 | 403 | 429 | 451 | 999) => verdict(Unknown, E::Blocked),
        Some(200..=299) => {
            let lower = body.to_lowercase();
            let visible = visible_lowercase(body);
            if BLOCKED_MARKERS.iter().any(|m| lower.contains(m)) {
                verdict(Unknown, E::Blocked)
            } else if CLOSED_MARKERS.iter().any(|m| visible.contains(m)) {
                verdict(Closed, E::ClosedMarker)
            } else if complete {
                verdict(Open, E::Ok)
            } else {
                verdict(Unknown, E::Error)
            }
        }
        Some(_) => verdict(Unknown, E::Error),
    }
}

const MAX_BODY_BYTES: usize = 512 * 1024;
const MAX_REDIRECT_HOPS: u8 = 2;

/// Fetch `url` (SSRF-guarded, no cookies/credentials, bounded redirects and body) and classify
/// it. Infallible by design: every failure is an `Unknown` verdict.
pub async fn check_url(url: &str) -> AvailabilityVerdict {
    if let Some(host) = reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
    {
        let limiter = crate::scraping::rate_limiter::for_host(&host).await;
        limiter.wait_for_slot().await;
        limiter.record_request().await;
    }
    let Ok(mut res) =
        crate::net::http::get_guarded_following_redirects(url, MAX_REDIRECT_HOPS).await
    else {
        return classify(None, "");
    };
    let status = res.status().as_u16();
    let mut body = Vec::new();
    let mut complete = false;
    while body.len() < MAX_BODY_BYTES {
        match res.chunk().await {
            Ok(Some(c)) => {
                let room = MAX_BODY_BYTES - body.len();
                body.extend_from_slice(&c[..c.len().min(room)]);
            }
            Ok(None) => {
                complete = true;
                break;
            }
            Err(_) => return classify(None, ""),
        }
    }
    classify_read(Some(status), &String::from_utf8_lossy(&body), complete)
}

#[cfg(test)]
mod tests;
