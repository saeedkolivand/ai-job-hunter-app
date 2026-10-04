//! Bookkeeping for a run: its status, the merge of its found jobs into the
//! record, and the later edits to those jobs. Split from `mod.rs` for the R8
//! module-size cap.

use std::collections::HashSet;

use super::cap;
use super::merge::{cluster_found_jobs, merge_found_jobs, merge_key};
use super::{derive_run_status, AutopilotStore, FoundJob, RunStatus};
use crate::db::now_ms;
use crate::scraping::cluster::new_cluster_count;

impl AutopilotStore {
    /// Set the most-recent-run outcome. `InProgress` is set at run start;
    /// `record_run` derives `Completed`/`CompletedWithErrors`/`Failed` from the
    /// per-board summaries for a run that reaches the record site.
    pub fn set_run_status(&self, id: &str, status: RunStatus) {
        let mut map = self.load();
        if let Some(ap) = map.get_mut(id) {
            ap.run_status = Some(status);
            ap.updated_at = now_ms();
        }
        self.save(map);
    }

    /// Set the run outcome for a run that has NO fresh summaries to report
    /// (never reached `record_run`) — ALSO clears `last_run_summaries` so a
    /// stale chip strip from the PRIOR run doesn't render as if it belonged to
    /// this one. Two callers today: an outright scrape error (`Failed`, no
    /// summaries were ever collected) and a user-cancelled run (`Completed`,
    /// the in-flight summaries were never finalized/recorded either).
    pub fn set_run_status_clearing_summaries(&self, id: &str, status: RunStatus) {
        let mut map = self.load();
        if let Some(ap) = map.get_mut(id) {
            ap.run_status = Some(status);
            ap.last_run_summaries = Vec::new();
            ap.updated_at = now_ms();
        }
        self.save(map);
    }

    /// Mark a run `Failed` on an outright scrape error. Thin wrapper over
    /// [`Self::set_run_status_clearing_summaries`] kept as a named entry point
    /// for the one fixed outcome that path always sets.
    pub fn fail_run_without_summaries(&self, id: &str) {
        self.set_run_status_clearing_summaries(id, RunStatus::Failed);
    }

    /// Reconcile runs left mid-flight: any autopilot still marked `InProgress`
    /// when the app starts was interrupted by a crash or close, so flip it to
    /// `Interrupted` for an honest badge instead of a stuck "running" state.
    /// Returns the ids reconciled, so the scheduler can schedule a single
    /// bounded recovery retry for the ones whose scheduled occurrence hasn't
    /// since rolled (see `autopilot_scheduler`). Called once at startup.
    pub fn mark_interrupted_runs(&self) -> Vec<String> {
        let mut map = self.load();
        let mut reconciled = Vec::new();
        for ap in map.values_mut() {
            if ap.run_status == Some(RunStatus::InProgress) {
                ap.run_status = Some(RunStatus::Interrupted);
                ap.updated_at = now_ms();
                reconciled.push(ap.id.clone());
            }
        }
        if !reconciled.is_empty() {
            self.save(map);
        }
        reconciled
    }

    /// Persist the outcome of a run: counts, last-run time, and the found-jobs
    /// list **merged** with prior runs by URL — so re-running keeps history
    /// (first-seen + any state) instead of replacing it, and genuinely new
    /// postings are flagged `is_new`.
    /// Returns the number of **newly surfaced** jobs in this run (postings whose
    /// URL was never seen before) — drives the "N new jobs" notification + tray.
    ///
    /// `summaries` are the per-board outcomes of the run: they are persisted on
    /// the record (so the UI can explain a zero/partial result after the run)
    /// and drive the derived [`RunStatus`] via [`derive_run_status`] — an
    /// all-boards-failed run now records `Failed`, a mixed run
    /// `CompletedWithErrors`, instead of a blanket `Completed`.
    pub fn record_run(
        &self,
        id: &str,
        total_found: u32,
        total_applied: u32,
        found_jobs: Vec<FoundJob>,
        summaries: Vec<crate::scraping::BoardScrapeSummary>,
        tombstones: &HashSet<(String, String)>,
        extra_agency: &[String],
    ) -> u32 {
        let mut map = self.load();
        let mut new_count = 0u32;
        if let Some(ap) = map.get_mut(id) {
            let now = now_ms();
            ap.total_found = total_found;
            ap.total_applied = total_applied;
            ap.found_jobs = merge_found_jobs(&ap.found_jobs, found_jobs);
            // Before clustering, so no cluster points at a job that's dropped.
            cap::cap_found_jobs(&mut ap.found_jobs);
            // Cross-board cluster the FULL merged list, write cluster annotations
            // onto each row, and count clusters whose members are ALL first-seen
            // this run (ADR-029 §f) — a known job resurfacing on another board no
            // longer notifies. `merge_found_jobs` set `is_new` per canonical key;
            // that set drives which clusters count as new.
            let new_keys: HashSet<String> = ap
                .found_jobs
                .iter()
                .filter(|j| j.is_new)
                .map(merge_key)
                .collect();
            let assignments = cluster_found_jobs(&mut ap.found_jobs, tombstones, extra_agency);
            new_count = new_cluster_count(&assignments, &new_keys);
            ap.run_status = Some(derive_run_status(&summaries));
            // Strip the Track B1 `health` before persisting. It is a DISPLAY-TIME
            // derivation of the live `board_health` store, not state belonging to
            // this run: freezing it here would (a) show a verdict that stopped
            // being true the moment the next run landed, and (b) leak it into the
            // backup bundle — `AutopilotStore::export` writes `lastRunSummaries`
            // verbatim, so importing on another machine would replay THIS
            // machine's failure streaks, timestamps, last error and run id as if
            // that machine had lived them. The store itself is deliberately not a
            // `DataStore` for exactly that reason; this closes the side door.
            ap.last_run_summaries = summaries
                .into_iter()
                .map(|mut s| {
                    s.health = None;
                    s
                })
                .collect();
            ap.last_run_at = Some(now);
            ap.updated_at = now;
        }
        self.save(map);
        new_count
    }

    /// Recompute + persist cluster annotations for ONE record's found-jobs after
    /// a tombstone change (`dedup_mark_not_duplicate`), leaving counts/run status
    /// untouched. No-op for an unknown id. The split takes effect immediately and
    /// — because clustering is recomputed every run — survives future re-scrapes.
    pub fn recompute_record_clusters(
        &self,
        id: &str,
        tombstones: &HashSet<(String, String)>,
        extra_agency: &[String],
    ) {
        let mut map = self.load();
        let mut changed = false;
        if let Some(ap) = map.get_mut(id) {
            cluster_found_jobs(&mut ap.found_jobs, tombstones, extra_agency);
            ap.updated_at = now_ms();
            changed = true;
        }
        if changed {
            self.save(map);
        }
    }

    pub fn stamp_last_run(&self, id: &str) {
        let mut map = self.load();
        if let Some(ap) = map.get_mut(id) {
            ap.last_run_at = Some(now_ms());
            ap.updated_at = now_ms();
        }
        self.save(map);
    }

    /// Patch `description` on every [`FoundJob`] — across EVERY autopilot
    /// record, not just one — whose own `url` normalizes to `normalized_url`
    /// (issue #1106 part b). `PostingsCache` (`postings::mod`) is a
    /// completely disjoint, session-lifetime store that a posting surfaced
    /// via `job`/`best-matches`/`autopilot_best_matches` never touches (those
    /// resources read `found_jobs` directly — see `agent_read`'s module
    /// doc); a correction from `commands::scrape::scrape_update_description`
    /// must reach this store too or the exact case issue #1106 reports (a
    /// correction that silently doesn't stick) stays unfixed. Same
    /// load-mutate-save shape as [`Self::recompute_record_clusters`] just
    /// above, but over every record and every matching row within it — the
    /// same posting can legitimately appear more than once across different
    /// autopilots. Returns the number of rows updated (`0` when nothing
    /// matched, so the caller can tell it found nothing to correct).
    ///
    /// Also recomputes `trust` (via [`crate::scraping::trust::assess_trust`],
    /// the same three-arg call `commands::autopilot::build_found_job` makes)
    /// — `trust` is genuinely description-scoped, so a correction from an
    /// empty/title-only description to a real one clears the stale
    /// `DescriptionUnavailable` flag immediately instead of leaving it
    /// contradicting the now-visible full text until the next scrape
    /// re-derives it (issue #1106).
    ///
    /// `score_provisional` is deliberately left UNTOUCHED here. It describes
    /// the `score` field (see `packages/shared/src/types/index.ts`'s doc
    /// comment on `MatchScoreSummary.provisional`), and — per the doc comment
    /// above — the numeric `score` itself is NOT recomputed by a manual text
    /// correction, so nothing about the flag's meaning has changed either. A
    /// prior version of this function derived `score_provisional` from the
    /// NEW description's blank-ness, which asserted freshness of a number
    /// that never moved (the exact dishonesty issue #1105 exists to close).
    /// It only goes back to `false` once an actual autopilot run re-scores
    /// the corrected content.
    pub fn update_found_job_descriptions(&self, normalized_url: &str, description: &str) -> u32 {
        let mut map = self.load();
        let mut updated = 0u32;
        for ap in map.values_mut() {
            let mut changed = false;
            for job in &mut ap.found_jobs {
                if crate::applications::normalize_job_url(&job.url) == normalized_url {
                    job.description = Some(description.to_string());
                    job.trust = Some(crate::scraping::trust::assess_trust(
                        &job.url,
                        &job.company,
                        description,
                    ));
                    changed = true;
                    updated += 1;
                }
            }
            if changed {
                ap.updated_at = now_ms();
            }
        }
        if updated > 0 {
            self.save(map);
        }
        updated
    }
}
