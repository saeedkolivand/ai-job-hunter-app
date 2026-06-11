---
status: accepted
---

# Application is the aggregate root; a Generation is a child Document

## Context

To add a user-mutable application status lifecycle (`saved → applied → screening →
interviewing → offer → accepted`, plus `rejected`/`ghosted`/`withdrawn`), we needed a
status-bearing entity. The existing `ai_generations` row already doubled as "the
application aggregate" — keyed and merged by `job_url` via `save_application()` /
`merge_application()`, carrying job link, board, answers, and the research brief
([apps/tauri/src-tauri/src/ai_generations/mod.rs](../../apps/tauri/src-tauri/src/ai_generations/mod.rs)).
But that row only exists once text has been generated, so it cannot represent a `saved`,
manually-tracked, or externally-submitted application that has **no** documents.

## Decision

Extract a dedicated **`applications`** table as the aggregate root (identity, status,
`applied_at`/`created_at`/`updated_at`, normalized `job_url`, board, company, title,
candidate, answers, brief, plus `notes`/`next_action_at`/`comp`/`contact_*`), with an
append-only **`status_events`** table for history. The `ai_generations` row is demoted to
a **pure child Document** (résumé/cover text, mode, languages) referencing its parent via
`application_id`. An Application may have zero child Generations (doc-less pursuit) or many
(one URL, separate résumé + cover actions).

## Considered options

1. **Extract Application; generations become children (chosen).** Single source of truth;
   supports doc-less Applications; aligns with the project's centralized-architecture rule
   (no parallel stores). Cost: largest migration — split schema, re-point
   `save_application`, `applied_job_urls`, the autopilot "applied" badge, and the
   `DataStore` export/import.
2. **New `applications` table, FK-link, leave `ai_generations` untouched.** Less churn, but
   the generation row keeps duplicate job/company/board fields and two records both call
   themselves "the aggregate" → denormalized, drift-prone — the parallel architecture the
   repo rules forbid.
3. **Add `status` onto `ai_generations`.** Smallest change, but every row still requires a
   generation, so `saved`/manual/external doc-less Applications are impossible — directly
   contradicts the feature's creation triggers.

## Consequences

- One migration set: add `ai_generations.application_id` → create `applications` +
  `status_events` → backfill one `Application(status=applied, applied_at=created_at)` per
  existing generation, link as child, seed a status event.
- "Applied" is redefined everywhere from "a generation exists for this URL" to
  "∃ Application(url) with status ≠ `saved`" (autopilot badge, `totalApplied`,
  `applied_job_urls`).
- Deleting an Application offers "remove tracking only (keep documents)" vs "delete
  everything"; deleting the last Generation leaves a doc-less Application with status
  intact.
- New top-level `/applications` route (Kanban + list) and a new `applications` IPC
  namespace (5-step capability flow).
