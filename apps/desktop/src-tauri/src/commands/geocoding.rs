//! Location autocomplete.
//!
//! **Offline first.** A bundled GeoNames index ([`geonames`]) answers virtually
//! every query with zero network traffic; a query it cannot match *exactly*
//! falls through to [`photon`] (photon.komoot.io — OpenStreetMap data).
//! Nominatim is no longer used: its usage policy explicitly forbids
//! autocomplete, and its rate limiting degraded the picker to "no suggestions"
//! exactly when a user typed fastest.
//!
//! The fallback is gated on match **quality**, not on the offline result being
//! empty. A prefix accident (`schweiz` prefix-matches Schweizer-Reneke, ZA)
//! looks like five confident rows, and `commands::autopilot` persists
//! `suggestion[0]`'s `countryCode` — so a weak hit must not veto the lookup
//! that would have said Switzerland.
//!
//! The suggestion shape (`{display, lat, lon, countryCode}`, city/country
//! granularity, max 5, deduped by label) is unchanged, so `geocode_suggest`,
//! the `LocationInput` picker, and autopilot's save-time `country_code`
//! backfill all keep working untouched.

use std::collections::{HashSet, VecDeque};
use std::sync::LazyLock;
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::{json, Value};

mod geonames;

#[cfg(test)]
mod test;

/// Build the offline index off the hot path. Called once from `lib.rs`'s setup
/// on a blocking thread — see [`geonames::warm`] for why lazy-on-first-use is
/// the wrong place to pay 60–250 ms of CPU.
pub(crate) fn warm_index() {
    geonames::warm();
}

/// Suggestions returned to the picker. Unchanged from the Nominatim era — the
/// dropdown is sized for it and callers rely on it.
const MAX_SUGGESTIONS: usize = 5;

/// Minimum query length before the *online* fallback may run for a query the
/// index could not match **at all**. A 1–2 char miss is almost always a
/// half-typed word, and firing a request per keystroke at a free community
/// endpoint is exactly the fair-use problem this rewrite exists to fix (the
/// renderer's 300 ms debounce in `packages/ui/src/hooks/useGeocoding.ts` is the
/// other half).
const MIN_ONLINE_QUERY_CHARS: usize = 3;

/// Higher floor for a query the index matched only *inexactly*. See
/// [`should_try_online`] — this is what keeps `ber`/`berl`/`berli` offline while
/// still letting a settled-looking `schweiz` reach Photon.
const MIN_WEAK_HIT_ONLINE_CHARS: usize = 6;

/// Nothing typed into a location field is longer than this; anything that is
/// would just become an oversized URL at a third party.
const MAX_ONLINE_QUERY_BYTES: usize = 200;

/// Reduce one Photon GeoJSON feature to a city- or country-level suggestion.
/// Returns None for anything more detailed (street/house/POI) that carries no
/// parent city, and for region/state-level hits — so the UI only ever shows
/// "City, Country" or a bare country name.
fn to_city_country(item: &Value) -> Option<Value> {
    let properties = item.get("properties");
    let string = |key: &str| {
        properties
            .and_then(|p| p.get(key))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    };

    let feature_type = string("type");
    let is_country_level = feature_type.as_deref() == Some("country");

    // `city` is present on sub-city features (street/house/district) and names
    // the containing city, so those collapse to it — the same rule the previous
    // Nominatim reduction used. A feature that IS a city carries its name in
    // `name` instead.
    let city = string("city").or_else(|| {
        matches!(
            feature_type.as_deref(),
            Some("city") | Some("district") | Some("locality")
        )
        .then(|| string("name"))
        .flatten()
    });

    let country = string("country");
    let country_code = string("countrycode").map(|s| s.to_uppercase());

    let coordinate = |i: usize| {
        item.get("geometry")
            .and_then(|g| g.get("coordinates"))
            .and_then(|c| c.get(i))
            .and_then(|v| v.as_f64())
    };
    // GeoJSON coordinate order is [lon, lat].
    let (lon, lat) = (coordinate(0), coordinate(1));

    // Keep only city-level matches, or explicit country-level matches.
    if city.is_none() && !is_country_level {
        return None;
    }

    let display = match (&city, &country) {
        (Some(city), Some(country)) => format!("{city}, {country}"),
        (Some(city), None) => city.clone(),
        // Country-level result (no city): label is the country name, falling
        // back to country_code if the country name is somehow absent.
        (None, Some(country)) => country.clone(),
        (None, None) => country_code.clone()?,
    };

    Some(json!({
        "display": display,
        "lat": lat,
        "lon": lon,
        "countryCode": country_code,
    }))
}

/// Keep the first suggestion per visible label, preserve order, cap at `limit`,
/// carrying an arbitrary tag alongside each kept suggestion (the offline index
/// tags each candidate with whether it matched a key exactly, so confidence can
/// be read off the rows that actually survive).
///
/// Lazy — the iterator is only pulled until `limit` distinct labels are seen,
/// which is what lets the offline index materialize JSON for the winners only.
fn dedupe_by_display_with<T>(
    items: impl Iterator<Item = (Value, T)>,
    limit: usize,
) -> Vec<(Value, T)> {
    let mut seen: HashSet<String> = HashSet::new();
    items
        .filter(|(s, _)| {
            s.get("display")
                .and_then(|d| d.as_str())
                .is_some_and(|d| seen.insert(d.to_string()))
        })
        .take(limit)
        .collect()
}

/// [`dedupe_by_display_with`] for callers with nothing to tag. One dedupe rule
/// for both the offline and the Photon path.
fn dedupe_by_display(items: impl Iterator<Item = Value>, limit: usize) -> Vec<Value> {
    dedupe_by_display_with(items.map(|value| (value, ())), limit)
        .into_iter()
        .map(|(value, ())| value)
        .collect()
}

/// Map a Photon `FeatureCollection` body to suggestions. Pure — split out from
/// the request so the mapping is unit-testable against canned JSON.
fn photon_suggestions(body: &Value) -> Vec<Value> {
    match body.get("features").and_then(|f| f.as_array()) {
        Some(features) => {
            dedupe_by_display(features.iter().filter_map(to_city_country), MAX_SUGGESTIONS)
        }
        None => vec![],
    }
}

/// Whether an offline result is worth a network round trip, and if so at what
/// query length. Pure (no network) so the fair-use guard is unit-tested
/// directly, mirroring autopilot's `should_derive_country_code`.
///
/// `weak` means the index returned rows but **none** of them matched a name
/// exactly — a prefix accident like `schweiz` → Schweizer-Reneke, ZA. Those get
/// a higher length floor rather than the zero-hit one: at 2–5 characters a
/// prefix hit is almost always what the user is typing toward (`ber` → Berlin),
/// so consulting Photon there would put a request behind nearly every partial
/// location entry — the fair-use problem this rewrite exists to fix.
fn should_try_online(query: &str, weak: bool) -> bool {
    // A pathological query would otherwise become a pathological third-party
    // URL; nothing legitimate in a location field is this long.
    if query.len() > MAX_ONLINE_QUERY_BYTES {
        return false;
    }
    // A query the bundled index structurally cannot hold skips the floors
    // entirely — they exist to avoid firing on a half-typed Latin word, and
    // applying them to a script the asset never carried just makes short
    // non-Latin input (`東京`, `москва`) permanently unanswerable.
    if !is_indexable_script(query) {
        return true;
    }
    let floor = if weak {
        MIN_WEAK_HIT_ONLINE_CHARS
    } else {
        MIN_ONLINE_QUERY_CHARS
    };
    query.chars().count() >= floor
}

/// Could this query match the bundled index at all?
///
/// The regeneration script keeps only Latin-script names (ASCII + Latin-1
/// Supplement + Latin Extended-A), and [`fold`](crate::scraping::cluster::normalize::fold)
/// leaves anything outside that untouched, so a query containing a letter from
/// another block can never equal or prefix a stored key. Note this deliberately
/// also covers Latin Extended-Additional (`Đà Nẵng`): those characters are
/// dropped by the same filter, so they too are only answerable online.
fn is_indexable_script(query: &str) -> bool {
    !query.chars().any(|c| {
        c.is_alphabetic() && !c.is_ascii_alphabetic() && !('\u{00C0}'..='\u{017F}').contains(&c)
    })
}

/// Photon's public endpoint. Split out so tests can drive the real request path
/// against a local mock server instead of the live service.
const PHOTON_ENDPOINT: &str = "https://photon.komoot.io/api/";

/// Interactive lookup: the user is waiting, but not forever.
const PHOTON_TIMEOUT: Duration = Duration::from_secs(5);

fn photon_url(endpoint: &str, query: &str) -> String {
    format!(
        "{endpoint}?q={}&limit=10&lang=en",
        urlencoding::encode(query)
    )
}

/// Online fallback: Photon (komoot), purpose-built for typeahead over
/// OpenStreetMap data. Never errors — a network/parse failure degrades to
/// `vec![]`, i.e. exactly the "no suggestions" state the offline miss already
/// produced. Endpoint and timeout are parameters so the request shape (URL,
/// User-Agent, timeout) is exercised by hermetic mock-server tests.
async fn photon_at(endpoint: &str, query: &str, timeout: Duration) -> Vec<Value> {
    let response = match crate::net::http::shared()
        .get(photon_url(endpoint, query))
        .header(reqwest::header::USER_AGENT, "ai-job-hunter/1.0")
        .timeout(timeout)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            // Debug, not warn: an offline laptop would otherwise log on every
            // keystroke. The picker degrades silently by design — this is the
            // only trace that says WHY it went quiet.
            tracing::debug!("geocode: photon request failed: {e}");
            return vec![];
        }
    };

    // A 4xx/5xx body is an error page, not a FeatureCollection. Checking the
    // status first turns "rate limited" into one clear line instead of a
    // confusing JSON-parse failure.
    let status = response.status();
    if !status.is_success() {
        tracing::debug!("geocode: photon returned {status}");
        return vec![];
    }

    match crate::net::http::read_json_capped::<Value>(
        response,
        crate::net::http::DEFAULT_MAX_BODY_BYTES,
    )
    .await
    {
        Ok(body) => photon_suggestions(&body),
        Err(e) => {
            tracing::debug!("geocode: photon body was not valid json: {e}");
            vec![]
        }
    }
}

/// Recent lookups, newest first. Small on purpose: a location field sees a
/// handful of distinct queries per session, and the picker re-issues the SAME
/// query constantly — every re-open of the dropdown, and every debounce tick
/// where the text stopped changing. Without this, a query whose best answer is
/// a weak offline hit (`amsterda`) re-asks Photon each time.
///
/// Only **non-empty** results are cached: a lookup that came back empty may
/// have been a transient network failure, and memoizing that would make the
/// picker permanently blank for that query.
static RECENT: LazyLock<RecentCache> =
    LazyLock::new(|| Mutex::new(VecDeque::with_capacity(RECENT_CAPACITY)));

/// `(query, suggestions)` pairs, most recently used first.
type RecentCache = Mutex<VecDeque<(String, Vec<Value>)>>;

const RECENT_CAPACITY: usize = 32;

fn cache_get(query: &str) -> Option<Vec<Value>> {
    let mut recent = RECENT.lock();
    let at = recent.iter().position(|(key, _)| key == query)?;
    // Move-to-front so the working set survives eviction.
    let entry = recent.remove(at)?;
    let hit = entry.1.clone();
    recent.push_front(entry);
    Some(hit)
}

fn cache_put(query: &str, suggestions: &[Value]) {
    if suggestions.is_empty() {
        return;
    }
    let mut recent = RECENT.lock();
    if recent.iter().any(|(key, _)| key == query) {
        return;
    }
    recent.push_front((query.to_string(), suggestions.to_vec()));
    recent.truncate(RECENT_CAPACITY);
}

/// The picker writes its own label back into the field, so the next time the
/// dropdown opens the query is a full `"City, Country"` string — which matches
/// no single index key. Retrying with the text before the first comma turns
/// that round trip back into an offline hit. `None` when there is no comma or
/// nothing before it.
fn before_comma(query: &str) -> Option<&str> {
    let (head, _) = query.split_once(',')?;
    let head = head.trim();
    (!head.is_empty()).then_some(head)
}

/// Core suggestion lookup shared by the `geocode_suggest` IPC command AND any
/// server-side caller (e.g. autopilot's save-time `country_code` derivation —
/// the autopilot aggregator zero-jobs fix). Empty query → empty result, no
/// lookup at all. Never errors.
///
/// The bundled index is consulted first and answers offline. The network is
/// touched only when the index has no **exact** name/code hit — an inexact hit
/// is a prefix accident (`schweiz` → Schweizer-Reneke, ZA) that must not veto
/// the fallback, because callers like autopilot persist `suggestion[0]`'s
/// country code. Offline results are still returned if Photon adds nothing.
pub(crate) async fn suggest(query: &str) -> Vec<Value> {
    suggest_at(PHOTON_ENDPOINT, query).await
}

/// [`suggest`] against an arbitrary Photon endpoint — the seam that makes the
/// fallback *and the merge rule* testable. Without it every `suggest` test
/// returns at the offline early-return, so "Photon replaces a weak offline hit"
/// and "an empty Photon answer leaves the offline rows alone" would be asserted
/// nowhere, and a future asset drift could silently start making live requests
/// from CI.
async fn suggest_at(endpoint: &str, query: &str) -> Vec<Value> {
    let query = query.trim();
    if query.is_empty() {
        return vec![];
    }
    if let Some(cached) = cache_get(query) {
        return cached;
    }

    // ~7-11 ms over ~34k pre-folded rows once the index is built (see
    // `geonames`), behind the picker's 300 ms debounce — no blocking pool.
    let mut hits = geonames::search(query, MAX_SUGGESTIONS);

    if !hits.exact {
        if let Some(head) = before_comma(query) {
            let retry = geonames::search(head, MAX_SUGGESTIONS);
            if retry.exact {
                hits = retry;
            }
        }
    }
    if hits.exact {
        cache_put(query, &hits.suggestions);
        return hits.suggestions;
    }

    if !should_try_online(query, !hits.suggestions.is_empty()) {
        return hits.suggestions;
    }
    // Photon REPLACES a weak offline result (a prefix accident is worse than a
    // real geocoder's answer), but an empty/failed one must never destroy the
    // rows we already have.
    let online = photon_at(endpoint, query, PHOTON_TIMEOUT).await;
    let result = if online.is_empty() {
        hits.suggestions
    } else {
        online
    };
    cache_put(query, &result);
    result
}

/// Whether `location` is non-empty after trimming — the only precondition for
/// attempting a geocode lookup. Pure (no network) so it's unit-tested directly.
fn should_derive_country_code(location: Option<&str>) -> bool {
    location.map(str::trim).is_some_and(|s| !s.is_empty())
}

/// Pick a country code out of the geocode service's ranked suggestions: the
/// first hit that actually CARRIES a VALID one wins (not just the first hit —
/// an earlier suggestion with an absent/null/malformed `countryCode` must not
/// block a usable later one), lower-cased to match
/// `BoardSearchInput::country_code`'s convention. Each candidate is validated
/// against the SAME 2-ASCII-letter shape `AutopilotTargetSchema.countryCode`
/// enforces (`^[A-Za-z]{2}$`) — a geocoded value written server-side bypasses
/// the IPC schema, so a `"USA"` / `"1a"`-style value is skipped, not accepted.
/// Pure — no network — so this is unit-tested directly. [`suggest`]'s own
/// lookup is covered too: offline against the real bundled index, and the
/// Photon round trip against a local mock server.
fn country_code_from_suggestions(suggestions: &[Value]) -> Option<String> {
    suggestions
        .iter()
        .find_map(|s| {
            s.get("countryCode")
                .and_then(|v| v.as_str())
                .filter(|cc| cc.len() == 2 && cc.bytes().all(|b| b.is_ascii_alphabetic()))
        })
        .map(str::to_lowercase)
}

/// Cap a server-side backfill lookup tighter than [`PHOTON_TIMEOUT`], which is
/// SHARED with the interactive `geocode_suggest` picker (where the user is
/// watching the dropdown and a longer wait is acceptable), so it can't be
/// lowered there. On a backfill path the lookup is best-effort — an autopilot
/// save / a scrape run should not stall on a slow third party.
///
/// Since the offline index landed this is a much rarer guard than it used to
/// be: a location the bundled index knows resolves in ~10 ms with no network at
/// all, so only a genuine miss (an unlisted endonym, a village below the 15k
/// population cut) can reach Photon and spend this budget. On timeout we fall
/// through to no suggestion (`None`), and the aggregator's guessed-market guard
/// still covers the residual case.
const BACKFILL_GEOCODE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Best-effort: when a search target has a real `location` but no
/// `country_code` (a prefilled or typed-freehand location that never went
/// through the picker — the aggregator zero-jobs bug), look one up via the SAME
/// geocode service the manual picker uses and return it for backfill.
///
/// Shared by both entry points that can receive a location without a picked
/// suggestion: autopilot save (`commands::autopilot`) and manual board scrape
/// (`commands::scrape`). Without it the aggregator hardcodes a `'de'` market
/// guess AND suppresses its sparse-city broadening, so e.g. a typed "Germany"
/// or "Amsterdam" search silently under-returns.
///
/// Never blocks or fails the caller: a network error / no match / timeout just
/// leaves `country_code` absent, exactly as it would without this fix — the
/// aggregator's own guessed-market guard (`scraping::boards::aggregator`)
/// covers that residual case too.
pub(crate) async fn derive_country_code(location: Option<&str>) -> Option<String> {
    if !should_derive_country_code(location) {
        return None;
    }
    // Safe: `should_derive_country_code` just proved this is `Some`.
    let location = location.unwrap_or_default().trim();
    // Cap the lookup at BACKFILL_GEOCODE_TIMEOUT (see const above): a timeout
    // yields an empty Vec -> None (best-effort), never a stalled caller.
    let suggestions = tokio::time::timeout(BACKFILL_GEOCODE_TIMEOUT, suggest(location))
        .await
        .unwrap_or_default();
    country_code_from_suggestions(&suggestions)
}

#[tauri::command]
pub async fn geocode_suggest(query: String) -> Value {
    json!(suggest(&query).await)
}
