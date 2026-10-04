//! Unit tests for the offline GeoNames index, the Photon fallback mapper, and
//! the server-side `country_code` backfill helpers
//! (`should_derive_country_code`, `country_code_from_suggestions`,
//! `derive_country_code`) — the backfill helpers live here, next to `suggest`,
//! because the manual scrape path and the autopilot save path share them.
//!
//! Everything here is hermetic: the index tests run against the **real bundled
//! asset** (that is the point — a silently-empty or mis-parsed asset must fail
//! the build, not degrade the picker), the fallback mapper is fed canned Photon
//! GeoJSON, and the request-shape tests drive a local `wiremock` server.
//! No test reaches the live network.
//!
//! This child module can reach the private parent helpers via `super::…`.

use std::time::Duration;

use serde_json::{json, Value};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{
    before_comma, country_code_from_suggestions, dedupe_by_display, derive_country_code, geonames,
    is_indexable_script, photon_at, photon_suggestions, should_derive_country_code,
    should_try_online, suggest, suggest_at, to_city_country, MAX_ONLINE_QUERY_BYTES,
    MAX_SUGGESTIONS,
};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn display(v: &Value) -> &str {
    v.get("display")
        .and_then(|d| d.as_str())
        .expect("display field missing")
}

fn country_code(v: &Value) -> Option<&str> {
    v.get("countryCode").and_then(|c| c.as_str())
}

fn lat(v: &Value) -> Option<f64> {
    v.get("lat").and_then(|l| l.as_f64())
}

fn lon(v: &Value) -> Option<f64> {
    v.get("lon").and_then(|l| l.as_f64())
}

fn displays(results: &[Value]) -> Vec<&str> {
    results.iter().map(display).collect()
}

fn search(query: &str) -> Vec<Value> {
    geonames::search(query, MAX_SUGGESTIONS).suggestions
}

/// Offline lookup including the match-quality flag the online fallback gates on.
fn search_hits(query: &str) -> geonames::Hits {
    geonames::search(query, MAX_SUGGESTIONS)
}

/// The country code of the first suggestion — what `commands::autopilot`'s
/// `country_code_from_suggestions` actually persists.
fn first_country_code(results: &[Value]) -> Option<&str> {
    results.first().and_then(country_code)
}

fn photon_feature(properties: Value, coordinates: Option<[f64; 2]>) -> Value {
    match coordinates {
        Some([lon, lat]) => json!({
            "type": "Feature",
            "geometry": { "type": "Point", "coordinates": [lon, lat] },
            "properties": properties,
        }),
        None => json!({ "type": "Feature", "properties": properties }),
    }
}

/// A local Photon stand-in answering every GET with `response`. Returns the
/// server (the caller keeps it alive) and the endpoint to point
/// `photon_at`/`suggest_at` at — never the live service.
async fn photon_mock(response: ResponseTemplate) -> (MockServer, String) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(response)
        .mount(&server)
        .await;
    let endpoint = format!("{}/api/", server.uri());
    (server, endpoint)
}

mod backfill;
mod index_build;
mod offline_index;
mod photon;
mod suggest;
