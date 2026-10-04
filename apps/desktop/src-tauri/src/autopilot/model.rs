//! The on-disk shape of an autopilot record (`autopilots.json`) and of the jobs a
//! run found: serde attributes, field names, defaults and back-compat
//! deserializers all live here, and they ARE the persisted format.

use serde::{Deserialize, Serialize};

use crate::scraping::cluster::ClusterMemberRef;

// ── Back-compat deserializer: `board` (string) OR `boards` (array) → Vec<String> ──

/// Accept either a JSON string (`"board": "linkedin"`) or a JSON array
/// (`"boards": ["linkedin","remotive"]`) and normalise to `Vec<String>`.
///
/// Backward-compatibility deserializer: on-disk `autopilots.json` records written
/// before the multi-board change store a single string under the legacy `"board"`
/// key; new records store an array under `"boards"`. The
/// `#[serde(alias = "board")]` on the field lets the old key name be accepted by
/// serde before this function is called — no data migration or rewrite required.
fn string_or_vec<'de, D>(de: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct StringOrVec;

    impl<'de> Visitor<'de> for StringOrVec {
        type Value = Vec<String>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a string or a sequence of strings")
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(vec![v.to_string()])
        }

        fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
            Ok(vec![v])
        }

        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut out = Vec::new();
            while let Some(s) = seq.next_element::<String>()? {
                out.push(s);
            }
            Ok(out)
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(Vec::new())
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(Vec::new())
        }

        fn visit_some<D2: serde::Deserializer<'de>>(self, d: D2) -> Result<Self::Value, D2::Error> {
            serde::Deserialize::deserialize(d).map(|v: serde_json::Value| match v {
                serde_json::Value::String(s) => vec![s],
                serde_json::Value::Array(arr) => arr
                    .into_iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect(),
                _ => Vec::new(),
            })
        }
    }

    de.deserialize_any(StringOrVec)
}

// ── Lenient deserializer: unrecognised workTypes entries are dropped ──────────

/// `Option<Vec<WorkType>>` that DROPS an unrecognised entry rather than
/// failing the whole field — mirrors [`string_or_vec`]'s back-compat posture,
/// one level down (an entry, not the field). A missing key deserializes as
/// `None` via `#[serde(default)]` on the field before this function is ever
/// called; an array left empty after dropping is `Some(vec![])`, which reads
/// as "no filter" the same as `None` does everywhere this is consumed.
fn parse_work_types_lenient<'de, D>(
    de: D,
) -> Result<Option<Vec<crate::scraping::types::WorkType>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: Option<Vec<String>> = Option::deserialize(de)?;
    Ok(raw.map(|values| {
        let parsed: Vec<crate::scraping::types::WorkType> =
            values.iter().filter_map(|s| s.parse().ok()).collect();
        let dropped = values.len() - parsed.len();
        if dropped > 0 {
            // Count only — never the raw unrecognised string (see
            // `AutopilotStore::load`'s own `[autopilot]`-prefixed warn just
            // above for the sibling convention). This is the ONLY trace a
            // future vocabulary rename leaves before a persisted autopilot's
            // filter silently widens to "any" (an all-dropped array reads as
            // `Some(vec![])`, identical to no filter — see
            // `BoardSearchInput::work_type_spec`).
            log::warn!(
                "[autopilot] dropped {dropped} unrecognised workTypes entr{} while \
                 loading a persisted target",
                if dropped == 1 { "y" } else { "ies" }
            );
        }
        parsed
    }))
}

// ── Data model ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotTarget {
    /// The boards to scrape. Accepts either a `"boards": [...]` array (new
    /// format) or a `"board": "..."` string (legacy on-disk format). The alias
    /// + custom deserializer normalise both to `Vec<String>` transparently.
    #[serde(alias = "board", deserialize_with = "string_or_vec")]
    pub boards: Vec<String>,
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
    /// Requested work arrangement(s) — empty/absent means "no filter" ("any").
    /// Unrecognised entries are DROPPED, not a hard parse failure (see
    /// [`parse_work_types_lenient`]). This matters on THREE paths, in
    /// increasing order of consequence:
    /// - `AutopilotStore::create`'s `unwrap_or_else` (a hard target parse
    ///   failure falls back to an empty target — only `work_types` would be
    ///   affected either way, since it is the one field that never hard-fails).
    /// - `AutopilotStore::update`'s patch merge silently ignores the WHOLE
    ///   target patch on a parse failure — every other edited field in the
    ///   same call is lost too, not just this one.
    /// - **The strongest reason, and the one that makes this load-bearing
    ///   rather than a nicety:** `AutopilotStore::load` deserializes each
    ///   persisted record individually and `filter_map`s out any that fail —
    ///   logged, never panicked — and the very next `save()` rewrites
    ///   `autopilots.json` WITHOUT that record. A single unrecognised
    ///   `workTypes` entry surviving to a hard parse failure would silently
    ///   delete an entire autopilot, including `found_jobs` and
    ///   `last_run_summaries`, on the next write. `#[serde(default)]` so a
    ///   pre-existing `autopilots.json` record with no `workTypes` key at
    ///   all — every record on disk today, since the only UI control that
    ///   could ever set this was removed in PR #614 before this field
    ///   existed — still deserializes.
    #[serde(
        default,
        deserialize_with = "parse_work_types_lenient",
        skip_serializing_if = "Option::is_none"
    )]
    pub work_types: Option<Vec<crate::scraping::types::WorkType>>,
    pub pages: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_filter: Option<String>,
    /// How many top-scoring postings autopilot should apply to after scraping.
    /// Defaults to 3 when absent.
    #[serde(default = "default_top_n")]
    pub top_n: u32,
    /// Watched-companies-only mode (ADR-030 §e): when `Some(true)`, a run resolves
    /// the user's currently-starred discovered companies at run time and scrapes
    /// only those per-ATS company slugs (instead of the curated seed). Additive +
    /// optional so old records deserialize unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watched_companies_only: Option<bool>,
}

pub(super) fn default_top_n() -> u32 {
    3
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotFilter {
    pub min_match_score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keywords: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_keywords: Option<Vec<String>>,
}

/// A job posting surfaced by an autopilot run. Lightweight summary persisted so
/// the user can review what each autopilot found.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundJob {
    pub title: String,
    pub company: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// Board id the posting was scraped from (its JobPosting.source). Persisted so the
    /// apply flow records accurate per-job provenance for multi-board autopilots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
    /// The board's own "this posting is remote" classification — copied from
    /// `JobPosting.extra["remote"]` at find-time, the SAME per-posting bit
    /// `scraping::engine::location_filter::location_verdict` treats as
    /// `board_remote` (its short-circuit BEFORE ever checking `location`
    /// text for a marker). A posting's `location` string is not always a
    /// reliable remote signal on its own — an all-remote board (WeWorkRemotely,
    /// RemoteOK, Remotive, Jobicy) may store `location: None` or a
    /// jurisdiction string like "USA Only" that carries no marker word at
    /// all — so a `remote` filter over `found-jobs` must consult this flag
    /// too, not `location` text alone (issue #1167 round-3 fix). `#[serde(default)]`
    /// so a record written before this field existed loads as `false`.
    #[serde(default)]
    pub board_remote: bool,
    /// Full job description — used to pre-fill a tailored resume/cover letter generation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Scraped salary range from the board (Adzuna only, today) — grounds the salary
    /// application answer before it falls back to a web lookup. `None` when the
    /// board doesn't expose salary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salary_min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salary_max: Option<f64>,
    /// ISO-4217 currency for `salary_min`/`salary_max`. `None` when the board didn't
    /// report one (e.g. an Adzuna market not in the country→currency map).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salary_currency: Option<String>,
    /// Match score (0–100) when the posting passed ranking; absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    /// The [`Self::score`] was computed over a TRUNCATED snippet (an aggregator/
    /// Adzuna description, which the board caps and the detail pane re-resolves
    /// to full text), so it may diverge from the detail pane's full-text
    /// re-score — the renderer marks such scores as provisional. `false` for
    /// full-text boards and for unscored jobs. Set at find-time in
    /// `commands::autopilot::build_found_job`. `#[serde(default)]` so a record
    /// written before this field existed loads as `false`.
    #[serde(default)]
    pub score_provisional: bool,
    /// WHICH kernel produced [`Self::score`] — the free keyword-coverage
    /// prefilter, or the combined semantic+ATS kernel the Jobs page uses
    /// (ADR-020 addendum). Set per job, so a posting whose embed failed and
    /// degraded back to keyword-only is labelled honestly even in a run where
    /// every other job re-ranked. Drives the renderer's metric label + band
    /// thresholds; `#[serde(default)]` makes every pre-existing record load as
    /// [`ScoreSource::Keyword`], which is exactly what it holds.
    #[serde(default)]
    pub score_source: ScoreSource,
    pub found_at: u64,
    /// The posting's publish-or-last-updated date (epoch ms), as reported by
    /// the board, copied from `JobPosting.posted_at` at find-time — distinct
    /// from [`Self::found_at`] (when WE scraped it). Most sources report a
    /// genuine creation/publish date (Adzuna, JSearch, the Apify LinkedIn
    /// actor, Arbeitnow, Ashby, Breezy, Jobicy, Lever, GermanTechJobs,
    /// BerlinStartupJobs, WeWorkRemotely, RemoteOK, Remotive, the HN "Who's
    /// Hiring" feed); a few (Jooble, Comeet, the Bundesagentur für Arbeit)
    /// only expose an "updated"/"current" timestamp upstream, so this can
    /// read as more recent than the posting's true original publish date for
    /// those. A board with no date field at all leaves it `None`.
    /// `#[serde(default)]` so a record written before this field existed
    /// loads as `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posted_at: Option<i64>,
    /// First surfaced in the most recent run (set by the dedup merge in
    /// [`AutopilotStore::record_run`]). Drives the "New" badge.
    #[serde(default)]
    pub is_new: bool,
    /// Whether the user has generated an application for this job. **Derived** at
    /// read time from a saved generation whose `job_url` matches `url` (see
    /// `commands::autopilot`), never hand-set, so it can't drift. Stored value is
    /// always `false`; the read path fills it in.
    #[serde(default)]
    pub applied: bool,
    /// Ghost-job trust signal, computed at find-time via
    /// [`crate::scraping::trust::assess_trust`]. `Option` (not a plain
    /// [`crate::scraping::trust::TrustAssessment`]) purely so a run recorded
    /// before this field existed still deserializes — `#[serde(default)]` gives
    /// `None` for a legacy record; every run recorded from here on always sets
    /// `Some(..)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust: Option<crate::scraping::trust::TrustAssessment>,
    /// Optional AI-reasoned note (2–4 sentences: why the job fits the résumé +
    /// one tailoring tip) generated for the top matches of a run when the
    /// autopilot has AI notes enabled (`assistant`). `None` for jobs not
    /// annotated (below the top-N ceiling, notes disabled, no provider, or the
    /// daily ceiling was hit mid-run). Read-only — never applied or submitted.
    /// `#[serde(default)]` so a job recorded before this field existed loads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_notes: Option<String>,
    // ── Cross-board cluster annotations (ADR-029) ──────────────────────────────
    // Recomputed at every `record_run` (and on a `dedup_mark_not_duplicate`
    // split) by the pure `scraping::cluster` pass; never hand-set. All
    // serde-defaulted so a record written before clustering existed loads
    // unchanged (`cluster_id` None, `cluster_canonical` true = standalone,
    // no members, not an agency).
    /// The cluster this job belongs to (the canonical member's `merge_key`).
    /// `None` only on a legacy record not yet re-clustered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cluster_id: Option<String>,
    /// Whether this job is its cluster's canonical (displayed) member. Defaults
    /// to `true` so a legacy/standalone job renders as its own canonical row.
    #[serde(default = "default_true")]
    pub cluster_canonical: bool,
    /// Every member of this job's cluster (`{key, board?, url}`), so the renderer
    /// can group + echo keys back to `dedup_mark_not_duplicate`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cluster_members: Vec<ClusterMemberRef>,
    /// Whether the posting's company is a recruiting/staffing agency (ADR-029 §i),
    /// computed at ingest from the built-in list + the user's extras.
    #[serde(default)]
    pub is_agency: bool,
}

/// Serde default for [`FoundJob::cluster_canonical`] — a job with no cluster
/// annotation reads as its own canonical row.
fn default_true() -> bool {
    true
}

/// Which scoring kernel produced a [`FoundJob::score`].
///
/// `Keyword` is the default in every sense: it is what the embedding-free
/// prefilter produces, what every record written before this enum existed
/// holds, and what a job degrades back to when its embed fails — so `Combined`
/// is only ever set by an explicit, successful semantic re-rank.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ScoreSource {
    /// Embedding-free keyword coverage (`documents::keywords::coverage_score`).
    #[default]
    Keyword,
    /// The Jobs-page combined semantic+ATS kernel
    /// (`commands::match_resume::score_one`).
    Combined,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AutopilotStatus {
    Active,
    Paused,
    Archived,
}

/// Outcome of the most recent run. Distinct from [`AutopilotStatus`] (the
/// agent's enabled/paused lifecycle): this tracks a single run so the UI can
/// show a live/failed/interrupted indicator.
///
/// `Interrupted` is not set by a run — it's reconciled at startup from a run
/// left `InProgress` when the app closed or crashed mid-run (see
/// [`AutopilotStore::mark_interrupted_runs`]).
///
/// `Completed`/`CompletedWithErrors`/`Failed` are derived from the run's
/// per-board summaries by [`derive_run_status`], so an all-boards-failed run no
/// longer masquerades as a clean `Completed, 0 found`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RunStatus {
    InProgress,
    Completed,
    /// The run finished and at least one board returned results, but at least
    /// one other board errored or kept only a partial (`truncated`) harvest.
    CompletedWithErrors,
    Failed,
    Interrupted,
}

/// Derive the honest outcome of a run from its per-board summaries.
///
/// - `Failed` — **zero** boards succeeded (every board errored or was skipped).
///   The run couldn't actually do its job, so it must not read as a clean
///   completion.
/// - `CompletedWithErrors` — at least one board succeeded, but at least one
///   other board errored or kept only a partial (`truncated`) harvest. Results
///   are real but incomplete.
/// - `Completed` — at least one board succeeded and none errored or truncated.
///   A `skipped` board alone does not downgrade the status: a skip
///   (`needs-login`/`needs-company`/`needs-keys`) is an expected no-op, not a
///   failure of a board that ran.
///
/// A board "succeeded" when it neither errored nor was skipped; a `truncated`
/// board counts as a partial success (it did return rows). An empty slice is
/// treated as `Completed` — nothing reported a problem — preserving the
/// pre-summaries behavior for the degenerate no-boards case.
pub(crate) fn derive_run_status(summaries: &[crate::scraping::BoardScrapeSummary]) -> RunStatus {
    if summaries.is_empty() {
        return RunStatus::Completed;
    }
    let succeeded = summaries
        .iter()
        .filter(|s| s.error.is_none() && s.skipped.is_none())
        .count();
    if succeeded == 0 {
        return RunStatus::Failed;
    }
    let any_incomplete = summaries
        .iter()
        .any(|s| s.error.is_some() || s.truncated.is_some());
    if any_incomplete {
        RunStatus::CompletedWithErrors
    } else {
        RunStatus::Completed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Autopilot {
    #[serde(rename = "_id")]
    pub id: String,
    pub name: String,
    pub status: AutopilotStatus,
    pub target: AutopilotTarget,
    pub filter: AutopilotFilter,
    pub schedule: String,
    /// Local clock hour (0–23) a recurring schedule fires at. Used by
    /// daily/twice_daily; ignored by hourly. `None` falls back to 09:00 in the
    /// scheduler. Defaulted so older persisted records load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule_hour: Option<u32>,
    /// Local clock minute (0–59) a recurring schedule fires at. Used by
    /// daily/twice_daily and as the "minute past the hour" for hourly. `None`
    /// falls back to minute 0 in the scheduler. Defaulted so older records load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule_minute: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_text: Option<String>,
    /// Optional base cover letter reused as the starting point when tailoring a
    /// found job in the apply assistant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_letter: Option<String>,
    /// Opt-in (Phase 4): after the keyword rank, attach a short AI-reasoned note
    /// to the top matches of each run. Read-only enrichment — never applies or
    /// submits anything. `#[serde(default)]` so existing records load as `false`.
    #[serde(default)]
    pub assistant: bool,
    /// Provider/model/base-URL snapshot the headless AI-notes run resolves through
    /// the centralized [`crate::pipeline::Completer`] (the same layer `ai_generate`
    /// uses). The scheduler has no renderer to read the active provider from, so the
    /// one chosen at opt-in time is persisted here. `None`/empty → notes skip
    /// gracefully for that run. Defaulted so older records load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_base_url: Option<String>,
    pub total_found: u32,
    pub total_applied: u32,
    /// Jobs surfaced by the most recent run. Defaulted so older records load.
    #[serde(default)]
    pub found_jobs: Vec<FoundJob>,
    /// Outcome of the most recent run. `None` until the first run. Drives the
    /// live/failed/interrupted badge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_status: Option<RunStatus>,
    /// Per-board outcome of the most recent run (board, count, error, skipped,
    /// truncated). Persisted so the UI can explain a zero/partial result *after*
    /// the run — until now these summaries were computed at the record site and
    /// discarded. `#[serde(default)]` so records written before this field
    /// existed load as an empty list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub last_run_summaries: Vec<crate::scraping::BoardScrapeSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_at: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}
