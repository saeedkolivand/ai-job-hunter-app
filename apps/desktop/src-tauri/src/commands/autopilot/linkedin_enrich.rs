//! Post-discovery LinkedIn description enrichment — the Tauri/orchestration
//! half of issue #1114 (LinkedIn slice). The pure "which URLs need a fetch" /
//! "what does a fetch outcome mean" decisions live one layer down, in
//! `autopilot_helpers::linkedin_enrich` (L2) — this file is L3 (`commands/`)
//! because it holds an `AppHandle`, calls the `scrape_update_description`
//! command, and drives the fetch loop; see that module's doc and
//! `docs/architecture-rules.md` R2/R7 for why the split sits here.

use std::collections::HashSet;
use std::sync::LazyLock;

use parking_lot::Mutex;
use tauri::AppHandle;

use crate::autopilot_helpers::linkedin_enrich::{classify_resolution, EnrichOutcome, EnrichTarget};
use crate::commands::scrape::DescriptionWriteStatus;
use crate::scraping::linkedin::rate_limiter::RateLimiter;

/// Process-global set of LinkedIn URLs currently being enriched by an
/// in-flight [`enrich_linkedin_descriptions`] pass. Keyed on URL alone, not
/// autopilot id: `select_linkedin_enrichment_targets` already dedupes by URL,
/// and the same LinkedIn posting URL is the same fetch target regardless of
/// which autopilot record surfaced it — a manual re-run, or a scheduler tick
/// landing while an earlier pass is still in flight, must not fetch the SAME
/// still-blank URL twice concurrently (wasted rate-limiter budget + duplicate
/// writes on a board this codebase already treats as soft-block-sensitive).
/// Mirrors `commands::autopilot::RUNS_IN_FLIGHT`'s shape: a per-process,
/// non-persisted dedup set that resets on restart — no in-flight enrichment
/// survives a restart anyway, so there is nothing to reconcile on launch.
static ENRICHING: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// Claim the subset of `urls` not already being enriched by a concurrent
/// pass, atomically inserting them into the shared in-flight set (one lock
/// acquisition) so a racing caller sees the claim immediately. Pure/sync —
/// unit-testable without an `AppHandle` or network.
fn claim_unclaimed(urls: Vec<String>) -> Vec<String> {
    let mut inflight = ENRICHING.lock();
    urls.into_iter()
        .filter(|url| inflight.insert(url.clone()))
        .collect()
}

/// Release `url` from the in-flight set. Called once per URL right after its
/// own fetch+write-back finishes — success OR failure, always — so a later
/// pass can retry a posting still blank after a failed attempt instead of
/// being stuck skipping it forever.
fn release_claim(url: &str) {
    ENRICHING.lock().remove(url);
}

/// Best-effort background pass: resolve each `url` via the shared single-URL
/// resolver, paced through LinkedIn's own process-wide rate limiter, and write
/// back any real description through the SAME mechanism
/// `scrape_update_description` uses for a manual correction (issue #1109's
/// write-back fix) — so the match scorer and every found-jobs surface pick up
/// the fetched text identically to a user-triggered fix.
///
/// Fire-and-forget: spawned via `tauri::async_runtime::spawn` from
/// `autopilot_run`, right after that run's own `record_run` call (before the
/// "new jobs" notification, though the ordering is moot either way — `spawn`
/// never blocks its caller), so a slow or failing LinkedIn fetch can never
/// delay the run's own "done" state or that notification. A per-URL failure
/// (network error, non-2xx, expired auth, selector drift returning no usable
/// text) is logged and skipped, never propagated — this is a best-effort
/// improvement layered on a scrape that already succeeded, and must never be
/// mistaken for a reason to retry aggressively (LinkedIn-hostile traffic
/// pattern) or to fail the run.
///
/// `urls` is first narrowed to [`claim_unclaimed`]'s result — a URL already
/// being enriched by a still-running pass (same autopilot re-run, or a
/// scheduler tick overlapping a manual one) is silently dropped here rather
/// than fetched a second time; it stays selectable by
/// `select_linkedin_enrichment_targets` on the NEXT run once this pass
/// releases it.
pub(super) async fn enrich_linkedin_descriptions(app: AppHandle, targets: Vec<EnrichTarget>) {
    let claimed: HashSet<String> = claim_unclaimed(targets.iter().map(|t| t.url.clone()).collect())
        .into_iter()
        .collect();
    let targets: Vec<EnrichTarget> = targets
        .into_iter()
        .filter(|t| claimed.contains(&t.url))
        .collect();
    if targets.is_empty() {
        return;
    }
    let limiter = crate::scraping::linkedin::rate_limiter::linkedin_rate_limiter();
    let mut enriched = 0usize;
    for target in &targets {
        if enrich_one(&app, limiter, target).await {
            enriched += 1;
        }
        release_claim(&target.url);
    }
    if enriched > 0 {
        log::info!(
            "[autopilot] LinkedIn description enrichment: {enriched}/{} postings updated",
            targets.len()
        );
    }
}

/// Fetch + (if usable) write back a single URL's description. Returns
/// whether the write-back actually updated a stored row — split out of
/// [`enrich_linkedin_descriptions`] so every exit path (skip, fetch error,
/// write-back error, success) funnels through one `return`, guaranteeing the
/// caller's [`release_claim`] always runs exactly once per URL.
async fn enrich_one(app: &AppHandle, limiter: &'static RateLimiter, target: &EnrichTarget) -> bool {
    limiter.wait_for_slot().await;
    let result = crate::scraping::scrape_url::resolve(&target.url).await;
    // Count the attempt against the shared budget regardless of outcome —
    // the request already reached LinkedIn's servers (or was denied one), so
    // a string of failures must still be paced, not retried in a burst.
    limiter.record_request().await;

    if let Err(ref e) = result {
        log::info!(
            "[autopilot] LinkedIn description enrichment failed for one posting: {}",
            crate::observability::sanitize_reason(&e.to_string())
        );
    }
    // Both write paths do synchronous file I/O — never call them inline on this
    // async task; `spawn_blocking` matches every other sync/blocking call this
    // crate makes from async code (e.g. `autopilot_best_matches`).
    let app_for_write = app.clone();
    let url_for_write = target.url.clone();
    let joined = match classify_resolution(target, result) {
        EnrichOutcome::Skip => return false,
        // Issue #1445: the url now leads to a different advert — hold the row
        // for review, never write the other ad's text onto it.
        EnrichOutcome::Drift(resolved) => {
            tauri::async_runtime::spawn_blocking(move || {
                crate::commands::scrape::flag_identity_drift(
                    &app_for_write,
                    &url_for_write,
                    &resolved,
                )
                .map(|_| DescriptionWriteStatus::IdentityDrift)
            })
            .await
        }
        EnrichOutcome::Description {
            description,
            resolved,
        } => {
            tauri::async_runtime::spawn_blocking(move || {
                crate::commands::scrape::scrape_update_description(
                    app_for_write,
                    crate::commands::scrape::ScrapeUpdateDescriptionRequest {
                        url: url_for_write,
                        description,
                        resolved,
                    },
                )
            })
            .await
        }
    };
    match joined {
        Ok(Ok(DescriptionWriteStatus::Updated)) => true,
        // The row moved/was dismissed between discovery and this fetch, or the
        // store-side verdict held it for review — nothing left to enrich.
        Ok(Ok(_)) => false,
        Ok(Err(e)) => {
            log::info!(
                "[autopilot] LinkedIn description write-back failed for one posting: {}",
                crate::observability::sanitize_reason(&e.to_string())
            );
            false
        }
        Err(e) => {
            log::info!(
                "[autopilot] LinkedIn description write-back task failed for one posting: {}",
                crate::observability::sanitize_reason(&e.to_string())
            );
            false
        }
    }
}

#[cfg(test)]
mod tests;
