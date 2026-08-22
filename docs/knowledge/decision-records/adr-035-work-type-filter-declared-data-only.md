# ADR-035: Work-type filter — declared data only, keep unknowns, validate before sending

Last updated: 2026-08-22

**Status:** Accepted

## Context

Job listings can express workplace type (remote / hybrid / on-site) in three forms: (1) an explicit API field declared by the board (Lever's `workplaceType`, Ashby's `workplaceType`); (2) a binary boolean field (Workable's `telecommuting`, Breezy's `is_remote`); (3) nothing at all (Greenhouse, LinkedIn, YCombinator). A naive filter would bridge the gaps with text inference (keyword matching in titles/descriptions), but keyword matching flips true on "this role is not remote" and "remote-first culture, 3 days in office." JobSpy (OSS library) has one `is_remote` field that means different things on different boards, illustrating the risk.

The UI already has per-board scraping since the introduction of company-scoped ATS boards. Surfacing work type requires deciding what happens when a board cannot or does not declare the value.

**Measurement facts (verified 2026-08-22):**

- **Lever:** 334 of 383 postings are `unspecified` → 87% Unknown
- **Freehire:** 72% of corpus carries no `work_mode` → 72% Unknown
- **Ashby:** 100% of rows have `workplaceType` declared (but `isRemote==true` means "Hybrid or Remote", affecting 79% of remote-looking rows)
- **SmartRecruiters:** Exact partition (3+36+328=367); every row has both `remote` and `hybrid` booleans (both-false = OnSite, precedence applies)
- **LinkedIn guest endpoint:** Facet-stripped for anonymous callers. Test: `f_WT=1` and `f_WT=2` overlapped 30/49 urns (measured, control arm `f_JT=F`/`f_JT=I` also overlapped → environment facet stripping, not `f_WT` fake, but consequence is the same: filter lies)

**Board coverage:**

- Most boards declare a workplace value on their public API (see the per-board table in `docs/SCRAPING_ENDPOINTS.md`)
- 4 boards are all-remote by definition (RemoteOK, Remotive, WWR, Jobicy)
- 9 boards have no workplace signal at all (Greenhouse, Personio, LinkedIn, YCombinator, Adzuna aggregator, and others)

## Decision

**Three core decisions:**

1. **Declare-only classification, no text inference.** Every board that declares `workType` / `workplaceType` / `remote`/`hybrid`/`on_site` booleans writes to `extra["workType"]`. Boards with no declaration emit nothing; the classifier reads `Unknown`. No keyword matching, no heuristics. If a board did not declare it at parse time, the filter cannot see it.

2. **`Unknown` is a value, never dropped.** When a filter like "show remote only" is requested, rows with `Unknown` workplace type are always kept. This is the majority state on key boards (Lever 87%, Freehire 72%). Omitting `Unknown` would silently discard 3 of 4 Lever results. Contradiction: "drop only on positive contradiction" is the policy for location filters; apply the same to work type.

3. **Never send an unvalidated filter param to a board.** Measured silent-ignore on 10 boards (Greenhouse, Lever, Ashby, Recruitee, Pinpoint, Arbeitnow, RemoteOK, Remotive, YCombinator, TheMuse). An unfiltered list is byte-for-byte indistinguishable from a filtered-and-matching-everything result, so a speculative param ships a lie. Only SmartRecruiters returns an error on invalid input. LinkedIn's guest endpoint is facet-stripped entirely (no `f_WT` send). Freehire's param is real but 72% undeclared, so pushing it upstream discards most of the board (local filter is better).

### Per-board mapping

| Board                        | Field                        | Action                                                                 |
| ---------------------------- | ---------------------------- | ---------------------------------------------------------------------- |
| Lever                        | `workplaceType`              | Normalize `onsite` (no hyphen) via `parse_work_type()`                 |
| Ashby                        | `workplaceType`              | Stop reading `isRemote` (true for both Hybrid+Remote)                  |
| SmartRecruiters              | `location.{remote,hybrid}`   | Upstream `&locationType=` (repeatable); exact partition                |
| Recruitee                    | `{remote,hybrid,on_site}`    | Precedence: hybrid > remote > on_site                                  |
| Workable                     | `telecommuting` bool         | Binary only on v1 endpoint                                             |
| Breezy                       | `location.is_remote` bool    | Absent = Unknown (not OnSite)                                          |
| Arbeitnow                    | `remote` bool (sparse)       | False = Unknown (not OnSite); title says "Hybrid" but has remote:false |
| Freehire                     | `work_mode` (already parsed) | Local filter only; 72% undeclared, pushing upstream discards them      |
| Pinpoint                     | `workplace_type`             | (Unverified live; marked for re-check)                                 |
| BambooHR                     | `locationType` "0"/"1"/"2"   | (Mapping inferred; marked for re-check)                                |
| Comeet                       | `workplace_type`             | (Docs-only; board hidden; marked for re-check)                         |
| RemoteOK/Remotive/WWR/Jobicy | —                            | Constant `remote` (all-remote by definition)                           |
| Others                       | —                            | `Unknown` (Greenhouse, Personio, LinkedIn, YCombinator, Adzuna)        |

### What is NOT built

- **Text inference.** No HYBRID_MARKERS, no "concrete city means on-site". Rejected.
- **Tier D skip reason.** RemoteOK/Remotive/WWR/Jobicy return constant `remote`, so filtering them out happens in the central post-filter. No dedicated `needs-work-type` skip (add if fetch volume justifies it).
- **LinkedIn `f_WT` param.** Guest endpoint is facet-stripped (measured: control arm also overlapped, not `f_WT` being fake). Authenticated path (with `li_at` cookie) untested. Do not send until re-measured with authentication.
- **Rippling, Arbeitsagentur, Workable upstream params.** All need endpoint version changes (v2, v6, v3 POST respectively). Left for a future PR.

### Classifier, engine, UI, i18n

| Layer      | Shape                                                                    | Notes                                                              |
| ---------- | ------------------------------------------------------------------------ | ------------------------------------------------------------------ |
| Classifier | `work_type_filter.rs`: `WorkTypeVerdict` enum                            | Remote / Hybrid / OnSite / Unknown                                 |
| Engine     | `keep_item` predicate composes both filters                              | Location mismatch + work-type mismatch (independent)               |
| Note       | `work-type-filtered:<n>` token                                           | Emitted unconditionally when work type requested                   |
| UI         | Three surfaces: manual search / autopilot wizard / jobs-page view filter | Multi-select, empty = "any"                                        |
| IPC        | `ScrapeRequest.workTypes`, `AutopilotTarget.workTypes`                   | `z.array(z.enum(WORK_TYPE_OPTIONS)).max(WORK_TYPE_OPTIONS.length)` |
| i18n       | `jobs.workType.*` nested keys                                            | Label, remote, hybrid, on-site, any, filterSummary                 |

## Alternatives considered

| Alternative                                                               | Why rejected                                                                                                                                                                                                     |
| ------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Text inference (keywords like "remote", "hybrid", "in-office", "on-site") | Produces false positives on negations ("not remote") and qualifiers ("remote-first culture, 3 days in office"). JobSpy's single `is_remote` field has different meaning per board. Risk outweighs coverage gain. |
| Hybrid classification from `remote` + `on_site` booleans when both false  | Fails on SmartRecruiters (both-false = OnSite, not Unknown); fails on Recruitee (both-false is common and should map to Unknown). Cannot encode both rules.                                                      |
| Send `f_WT` to LinkedIn's guest endpoint                                  | Guest endpoint is facet-stripped for anonymous callers (measured: `f_WT=1` and `f_WT=2` overlapped 30/49; control arm also overlapped). Creates a filter that lies.                                              |
| Push `work_mode` filter to Freehire upstream                              | 72% of corpus undeclared. Pushing upstream silently discards 72% of the board. Local filter keeps the Unknown rows.                                                                                              |
| Persist a `work_type` candidate preference on `job_preferences`           | Column was deliberately dropped by `drop_unused_job_preferences_columns` migration (ADR-013 deferral). Resurrect only if we have new use case (none today).                                                      |
| Two-phase training: measure prevalence, then choose depth by board        | Premature optimization. Enough boards declare a value to launch on declared data alone. Text inference can be added post-launch if needed (it won't be).                                                         |

## Consequences

- **Majority-Unknown is kept.** Lever (87% unspecified), Freehire (72% undeclared) users can now filter by the 13% and 28% they do declare, with the rest kept for fallback matching.
- **SmartRecruiters narrows upstream.** Only verified board that accepts the filter param validates it. Saves network on large companies (Cloudflare, Uber, etc. with 1000s of postings).
- **LinkedIn filter not exposed.** The guest endpoint is facet-stripped for anonymous callers (measured twice; the `f_JT` control arm failed identically). The cookie'd path is a different cell and is UNTESTED — do not assume it behaves the same. LinkedIn postings therefore read as `Unknown` and are always kept, so the cost is a filter that does not narrow LinkedIn, not results going missing.
- **Text inference deferred.** Can be added post-launch; no refactoring needed (classifier boundary is clean). Inferred fields would write to `extra.workType` same as declared fields.
- **Note-slot contention fixed.** `BoardScrapeSummary.note` widened to `notes: Vec<String>` so both `location-filtered` and `work-type-filtered` can coexist on one board (precedence: board-native, location, work-type, aggregator).

## Related

- `apps/desktop/src-tauri/src/scraping/engine/work_type_filter.rs` — classifier
- `apps/desktop/src-tauri/src/scraping/engine/mod.rs` — engine wiring
- `apps/desktop/src-tauri/src/scraping/boards/*/mod.rs` — per-board `extra["workType"]` writes
- `packages/shared/src/schemas/index.ts` — `WORK_TYPE_OPTIONS`, IPC schema arrays
- `docs/SCRAPING_ENDPOINTS.md` — live measurements, per-board field names
- `docs/knowledge/scraping-domain.md` — work-type section with design detail + policy
