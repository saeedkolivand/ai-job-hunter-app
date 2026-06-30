# ADR-007: `ai_generations` as the application aggregate with merge-upsert by job URL

Last updated: 2026-06-01

**Status:** Accepted

## Context

A single job application involves multiple separate AI actions: résumé tailoring, cover-letter generation, application-question answers, and company-research brief. Storing each as an independent row would scatter them across the table and make it impossible to query "everything generated for this application" without a join.

The `applied` status of a job (whether the user has generated documents for it) was previously stored explicitly — creating a dual-write risk if a save succeeded but the status update failed.

## Decision

`ai_generations` is the **application aggregate**: one row per job URL. `save_application` in `apps/desktop/src-tauri/src/ai_generations/mod.rs` performs a **merge-upsert by `job_url`**: when an incoming record shares a `job_url` with an existing row, it calls the pure `merge_application` function to combine fields (résumé text, cover-letter text, answers, brief) into the existing row rather than inserting a new one. Records without a `job_url` (manual generations) always insert as fresh rows.

`applied` status is **derived**, not stored: `applied_job_urls()` queries `DISTINCT job_url` from the table and returns the set. Schema additions (e.g. `application_answers` column, `job_link` column) are additive migrations — no destructive column changes.

## Consequences

- One row in `ai_generations` = one full application bundle; easy to query, audit, and export.
- `applied` derivation eliminates dual-write risk; autopilot reads `applied_job_urls()` to skip already-applied jobs.
- `merge_application` is a pure function, independently testable (`ai_generations/test.rs`).
- New fields on the aggregate require an additive migration; the lockstep comment in `commands/data.rs::build_bundle` must be updated to include any new persistent fields in exports/backups.

## Addendum — field-selective text edit (F1, v0.65)

`AiGenerationsContract.update(req: AiGenerationUpdateRequest)` (`aiGenerations:update` channel) allows post-save editing of `resumeText` / `coverLetterText` without touching the rest of the aggregate. Rust: `update_texts` in `ai_generations/mod.rs` — must verify `rows_changed > 0` and error on 0 (silent Ok diverges the optimistic cache). Frontend optimistic path: `useUpdateAiGeneration` in `renderer/services/use-ai-generations/`; no re-sync `useEffect` — the optimistic cache owns truth, rollback handles failure. See `AiGenerationUpdateRequest` in `packages/shared/src/ipc/contracts/aiGenerations.ts`.
