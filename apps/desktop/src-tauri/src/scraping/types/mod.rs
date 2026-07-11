/// Shared types for scraping operations.
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobPosting {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "externalId")]
    pub external_id: Option<String>,
    pub title: String,
    pub company: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub url: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirements: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "postedAt")]
    pub posted_at: Option<i64>,
    #[serde(rename = "capturedAt")]
    pub captured_at: i64,
    /// Board-specific metadata (salary, remote status, etc.)
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct BoardSearchInput {
    pub query: String,
    pub location: Option<String>,
    /// Target number of postings to collect (the user's requested item count,
    /// clamped to 100). The engine caps streamed/returned items to this centrally.
    pub amount: u32,
    /// Per-board page request BUDGET. Each board clamps this down to its own max
    /// page count; it bounds how many requests we make, independent of `amount`.
    pub pages: u32,
    pub date_filter: Option<String>,
    pub job_type: Option<String>, // 'F' (Full-time), 'P' (Part-time), etc.
    pub work_type: Option<String>, // '1' (On-site), '2' (Remote), '3' (Hybrid)
    pub experience_level: Option<String>,
    pub easy_apply: Option<bool>,
    pub actively_hiring: Option<bool>,
    pub verified: Option<bool>,
    pub sort_by: Option<String>, // 'DD' (Date Descending), 'R' (Relevance)
    // Structured location from a picked geocode suggestion (#49/#40). Boards that
    // support precise geo filtering (e.g. LinkedIn geoId + distance) use these;
    // boards without geo filtering ignore them.
    pub country_code: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub radius_km: Option<u32>,
    /// Company / board identifiers for ATS boards (greenhouse, lever, ashby,
    /// recruitee, personio, smartrecruiters, pinpoint, rippling, breezy,
    /// bamboohr) whose public APIs require a company slug instead of a global
    /// keyword search. Empty = no company filter; only those ATS boards read
    /// it, every other board ignores it.
    pub companies: Vec<String>,
}

/// Canonical structured location for a search — the single model the engine's
/// central location post-filter (and location-aware boards) reason over,
/// assembled once from the free-text `location` plus the structured geo fields a
/// picked geocode suggestion carries. All fields are optional so a partial or
/// absent location degrades gracefully; a `None` spec (see
/// [`BoardSearchInput::location_spec`]) means "no location was requested", which
/// keeps location-agnostic searches byte-identical. Serde so it can ride IPC /
/// be persisted; `#[serde(default)]` on every field so a sparse or older payload
/// deserializes cleanly.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationSpec {
    /// City / place name. Today sourced from the free-text `location`; a
    /// structured geocode city can populate it later.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    /// Region / state, when a structured pick provides one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// ISO 3166-1 alpha-2 country code — routes the aggregator's market directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latitude: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub longitude: Option<f64>,
    /// Search radius in km — consumed by LinkedIn's `distance` param.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius_km: Option<u32>,
}

impl LocationSpec {
    /// True when the spec carries no location signal at all.
    pub fn is_empty(&self) -> bool {
        self.city.is_none()
            && self.region.is_none()
            && self.country_code.is_none()
            && self.latitude.is_none()
            && self.longitude.is_none()
            && self.radius_km.is_none()
    }
}

impl BoardSearchInput {
    /// The canonical structured location for this search, assembled from the
    /// free-text `location` (as the city) plus the structured geo fields. Returns
    /// `None` when no location signal was supplied, so location-agnostic searches
    /// stay byte-identical (the engine's central location post-filter is inert for
    /// a `None` spec). The free-text `location` field is preserved alongside for
    /// boards that only understand text.
    pub fn location_spec(&self) -> Option<LocationSpec> {
        let city = self
            .location
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let spec = LocationSpec {
            city,
            region: None,
            country_code: self.country_code.clone(),
            latitude: self.latitude,
            longitude: self.longitude,
            radius_km: self.radius_km,
        };
        (!spec.is_empty()).then_some(spec)
    }
}

pub struct ScrapeContext {
    pub signal: tokio_util::sync::CancellationToken,
    pub on_progress: Option<Box<dyn Fn(f32) + Send>>,
    /// Streamed per-posting sink. See [`Scraper::search`]'s doc contract: the
    /// set streamed here must equal the `search` return value's set.
    pub on_item: Option<Box<dyn Fn(JobPosting) + Send>>,
    /// Reports that a paginated board stopped early after a mid-run page failure
    /// and kept its partial harvest (e.g. `"page 3 of 5 failed: HTTP 429"`). Only
    /// the paginated boards that fail-open on a later page (The Muse, Arbeitnow,
    /// Arbeitsagentur) set this, via [`ScrapeContext::report_truncation`]; the
    /// engine surfaces it as `BoardScrapeSummary.truncated` so a partial harvest
    /// is distinguishable from a complete one. `None` for every other board and
    /// whenever no sink is wired (e.g. a board's own unit tests).
    pub on_truncation: Option<Box<dyn Fn(String) + Send>>,
    /// Per-board **informational** side-channel for a location policy the board
    /// applied that the user didn't explicitly ask for — currently the aggregator's
    /// guessed market (no `country_code` supplied) or a sparse city search widened
    /// country-wide. The engine surfaces it as `BoardScrapeSummary.note`. Unlike
    /// [`Self::on_truncation`] this is an `Arc`, not a `Box`: the aggregator hands
    /// it to a sub-provider (`AdzunaProvider`) that holds it across `.await`, so it
    /// must be `Send + Sync`. `None` when the board reports no policy note and
    /// whenever no sink is wired (e.g. a board's own unit tests).
    pub on_note: Option<std::sync::Arc<dyn Fn(String) + Send + Sync>>,
}

impl ScrapeContext {
    /// Report a partial-harvest truncation reason (see [`ScrapeContext::on_truncation`]).
    /// No-op when no sink is wired, so paginated boards can report unconditionally.
    pub fn report_truncation(&self, reason: String) {
        if let Some(ref cb) = self.on_truncation {
            cb(reason);
        }
    }

    /// Report an informational location-policy note (see [`ScrapeContext::on_note`]).
    /// No-op when no sink is wired, so a board can report unconditionally.
    pub fn report_note(&self, note: String) {
        if let Some(ref cb) = self.on_note {
            cb(note);
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScraperMode {
    Http,
    Browser,
}

/// Whether a board can be scraped without a logged-in session. Serializes to
/// `"guest" | "optional" | "required"` for the renderer's manual jobs picker.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthRequirement {
    /// No login needed — guest scraping returns full results.
    Guest,
    /// Guest scraping works; logging in enriches results.
    Optional,
    /// Guest scraping returns ~nothing — login is mandatory.
    Required,
}

#[allow(dead_code)]
#[async_trait]
pub trait Scraper: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn mode(&self) -> ScraperMode;

    /// Auth tier for this board. Defaults to [`AuthRequirement::Guest`] so a
    /// board only overrides this when it needs a session.
    fn auth(&self) -> AuthRequirement {
        AuthRequirement::Guest
    }

    /// Whether the board appears in the manual jobs picker. Defaults to `true`;
    /// a board overrides to `false` to stay registered (dispatchable) but hidden.
    fn listed(&self) -> bool {
        true
    }

    /// Whether this board requires a company slug to return any results.
    ///
    /// ATS platforms (Greenhouse, Lever, Ashby, Recruitee, Personio,
    /// SmartRecruiters, Pinpoint, Rippling, Breezy HR, BambooHR, Workable) have
    /// no global keyword search — their public APIs only accept a per-company
    /// slug. Boards that return `true` here are skipped by the engine with
    /// reason `"needs-company"` when `input.companies` is empty, instead of
    /// making a wasted network call that would return nothing.
    ///
    /// Defaults to `false` so only the 11 ATS boards need to override this.
    fn requires_company(&self) -> bool {
        false
    }

    /// Whether this board needs API keys/credentials that are currently absent.
    ///
    /// Some boards (the aggregator, backed by Adzuna/JSearch/Apify) return
    /// nothing without configured API keys. Rather than run and yield a silent
    /// empty result, the engine skips such a board with reason `"needs-keys"`
    /// when this returns `true`, so the UI/diagnostics can prompt the user to
    /// configure their keys instead of showing an unexplained zero.
    ///
    /// This is evaluated per scrape (it may read the credential store), so a key
    /// added in Settings clears the skip on the next run without a restart.
    ///
    /// Defaults to `false` so only key-backed boards override it.
    fn needs_keys(&self) -> bool {
        false
    }

    /// Whether this board narrows results by the requested location SERVER-SIDE
    /// (the query itself is scoped to the place before results return), as opposed
    /// to ignoring location or only filtering it client-side.
    ///
    /// When this is `false` and a location was requested, the engine applies a
    /// conservative central post-filter to this board's results (drops only
    /// postings whose OWN location clearly mismatches; never remote or
    /// unknown-location rows). Verified per board by reading each `search()`:
    /// - aggregator: Adzuna/JSearch `where` param + country market routing → `true`
    /// - linkedin: geoId typeahead + `distance`/radius params → `true`
    /// - arbeitsagentur: `wo` (where) param → `true`
    ///
    /// Every other board — remote-only feeds, regional feeds, company-slug ATS,
    /// and boards that only filter location client-side — returns the default
    /// `false`.
    fn supports_location(&self) -> bool {
        false
    }

    /// **Contract:** every posting streamed via `ctx.on_item` must also appear in
    /// the returned `Vec` (and vice versa) — the same set, not a subset/superset.
    /// The engine relies on this identity: under a live location filter it
    /// returns the streamed/kept set as the board's authoritative result, so a
    /// board that streams a different set than it returns would silently lose or
    /// fabricate results for that board.
    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> Result<Vec<JobPosting>, anyhow::Error>;

    /// Reads as "scrape *from* a URL" — not a constructor, so the `&self` receiver
    /// is intentional despite the `from_` prefix.
    #[allow(clippy::wrong_self_convention)]
    async fn from_url(
        &self,
        _url: &str,
        _ctx: ScrapeContext,
    ) -> Result<Option<JobPosting>, anyhow::Error> {
        Ok(None)
    }
}

#[cfg(test)]
mod test;
