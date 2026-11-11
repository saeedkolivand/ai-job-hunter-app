//! v6 list response types and the mapping helpers over them.

use crate::error::AppError;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct Adresse {
    pub(super) ort: Option<String>,
    pub(super) region: Option<String>,
    pub(super) land: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Stellenlokation {
    pub(super) adresse: Option<Adresse>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Zeitraum {
    pub(super) von: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Stellenangebot {
    pub(super) referenznummer: String,
    #[serde(rename = "stellenangebotsTitel")]
    pub(super) titel: Option<String>,
    pub(super) hauptberuf: Option<String>,
    pub(super) firma: Option<String>,
    pub(super) stellenlokationen: Option<Vec<Stellenlokation>>,
    /// Current publication window; `von` is the (re)publication date.
    pub(super) veroeffentlichungszeitraum: Option<Zeitraum>,
    /// `YYYY-MM-DD`; the FIRST publication, so old for republished listings.
    #[serde(rename = "datumErsteVeroeffentlichung")]
    pub(super) erste_veroeffentlichung: Option<String>,
    #[serde(rename = "externeURL")]
    pub(super) externe_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ListResp {
    /// Absent (not empty) both for a genuine zero-hit search and for a changed
    /// response shape; `maxErgebnisse` tells the two apart.
    pub(super) ergebnisliste: Option<Vec<Stellenangebot>>,
    #[serde(rename = "maxErgebnisse")]
    pub(super) max_ergebnisse: Option<i64>,
}

impl ListResp {
    pub(super) fn is_shape_drift(&self) -> bool {
        self.ergebnisliste.is_none() && self.max_ergebnisse != Some(0)
    }

    /// Page-1 gate: a changed response shape is an error, not "found nothing".
    pub(super) fn check_first_page(&self) -> Result<(), AppError> {
        if self.is_shape_drift() {
            return Err(AppError::Parse(
                "arbeitsagentur list response has no `ergebnisliste`".into(),
            ));
        }
        Ok(())
    }
}

impl Stellenangebot {
    /// Prefer the current publication date over the first one (republished
    /// listings would otherwise look months old and fail recency filters).
    pub(super) fn posted_at_ms(&self) -> Option<i64> {
        let d = self
            .veroeffentlichungszeitraum
            .as_ref()
            .and_then(|z| z.von.as_deref())
            .or(self.erste_veroeffentlichung.as_deref())?;
        let d = chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()?;
        Some(d.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis())
    }
}

/// v6 writes region/country upper-case ("BERLIN", "DEUTSCHLAND"); show them
/// title-cased. Keeps the country text so the location post-filter (which
/// tokenizes the whole location string, case-insensitively) still sees it.
pub(super) fn title_case(s: &str) -> String {
    let s = s.replace('_', "-");
    let mut out = String::with_capacity(s.len());
    let mut start = true;
    for c in s.chars() {
        if start {
            out.extend(c.to_uppercase());
        } else {
            out.extend(c.to_lowercase());
        }
        start = !c.is_alphanumeric();
    }
    // v6 transliterates ("BADEN_WUERTTEMBERG"); restore the two spellings that
    // occur in region names. The location filter folds ue/ü either way.
    out.replace("Wuerttemberg", "Württemberg")
        .replace("Thueringen", "Thüringen")
}

/// "Berlin, Berlin, DEUTSCHLAND" -> "Berlin, Deutschland": region is dropped
/// when it repeats the city (ignoring case).
pub(super) fn format_location(a: Option<&Adresse>) -> String {
    let Some(a) = a else { return String::new() };
    let ort = a.ort.as_deref().filter(|s| !s.is_empty());
    let region = a
        .region
        .as_deref()
        .filter(|r| !r.is_empty() && ort.is_none_or(|o| !o.eq_ignore_ascii_case(r)));
    let land = a.land.as_deref().filter(|s| !s.is_empty());
    [
        ort.map(str::to_string),
        region.map(title_case),
        land.map(title_case),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(", ")
}
