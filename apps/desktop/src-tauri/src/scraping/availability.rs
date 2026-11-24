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

/// Pure verdict from an HTTP status (`None` = no response: timeout / network error) and body.
pub fn classify(status: Option<u16>, body: &str) -> AvailabilityVerdict {
    use Availability::{Closed, Open, Removed, Unknown};
    use AvailabilityEvidence as E;
    match status {
        None => verdict(Unknown, E::Error),
        Some(404 | 410) => verdict(Removed, E::HttpGone),
        Some(401 | 403 | 429 | 451 | 999) => verdict(Unknown, E::Blocked),
        Some(200..=299) => {
            let lower = body.to_lowercase();
            if BLOCKED_MARKERS.iter().any(|m| lower.contains(m)) {
                verdict(Unknown, E::Blocked)
            } else if CLOSED_MARKERS.iter().any(|m| lower.contains(m)) {
                verdict(Closed, E::ClosedMarker)
            } else {
                verdict(Open, E::Ok)
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
    while body.len() < MAX_BODY_BYTES {
        match res.chunk().await {
            Ok(Some(c)) => body.extend_from_slice(&c),
            Ok(None) => break,
            Err(_) => return classify(None, ""),
        }
    }
    classify(Some(status), &String::from_utf8_lossy(&body))
}

#[cfg(test)]
mod tests;
