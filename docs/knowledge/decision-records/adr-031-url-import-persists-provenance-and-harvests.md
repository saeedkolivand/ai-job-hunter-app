# ADR-031 — URL import persists provenance and feeds slug harvesting

**Status:** Accepted  
**Date:** 2026-07-21  
**Deciders:** repo owner, main session

## Context

The `JobUrlImport` component (in `components/job/JobUrlImport`) has shipped inside `JobAdField` and is callable from the AI Generate step. It invokes the synchronous `scrape.resolveUrl` IPC command to resolve a single-posting URL.

However, two critical gaps emerged in integration with the broader system:

1. **Lost provenance**: When a generation originates from URL import, the renderer's persist path (`features/ai-generate/hooks/useGeneration.ts::persist()`) does not set the already-existing `AiGenerationSaveRequest.jobUrl` and `board` fields. This breaks applied-detection joins (which derive `FoundJob.applied` from `ai_generations.job_url`) and loses cluster-provenance data for ADR-029.

2. **No harvesting integration**: The URL resolve path does not feed into ADR-030's slug harvesting seam, leaving one import path disconnected from autocomplete aggregation.

3. **Failure UX**: The unreachable/unparseable URL and rate-limit error paths need audit against the never-a-dead-end rule.

The backlog row in `docs/ARCHITECTURE_STATUS.md` ("URL-to-job-ad extraction in AI Generate") tracks this.

## Decision

### (a) Contract: `scrape.resolveUrl` remains the entry point

No new IPC command. The batch spec's "wire scrape.url" resolves to `scrape.resolveUrl` (which is single-posting synchronous), not the streaming multi-board `scrape.url` job. Granularity is correct.

### (b) Renderer persists provenance fields end-to-end

When a generation originates from a URL import, the renderer's job-ad state holds the source `url` and detected `board`. The persist path (`useGeneration.ts::persist()`) populates the existing `AiGenerationSaveRequest.jobUrl` and `board` fields when saving. No schema change; the fields existed end-to-end and were simply not wired.

### (c) Rust core feeds the harvest seam

The Rust `scrape_resolve_url` command (in `commands/scrape.rs`) calls the ADR-030 harvest entry point (`commands/discovery.rs::posting_to_ref/harvest` seam, source 'scrape') on a successful resolve. This is one call site reusing the existing pure extractor; zero new network calls, no retry logic duplication.

### (d) Failure path respects the never-a-dead-end contract

- Unreachable/unparseable URL displays the existing `jobUrlImport.*` i18n error inline.
- Manual paste fallback remains usable.
- Rate-limit (429) respects the existing backoff path.
- English + German parity verified.
- Keyboard-only flow tested.

## Alternatives rejected

- **Switching UI to streaming `scrape.url`**: wrong granularity for single-URL import; worse UX.
- **New dedicated IPC**: the existing contract already returns the posting; YAGNI.
- **Harvesting in the renderer**: business logic stays in Rust core.

## Consequences

- URL-imported generations now join the applied-detection and cluster-provenance joins like autopilot/tailor flows.
- Every URL import feeds the slug autocomplete for free.
- Closes the `docs/ARCHITECTURE_STATUS.md` backlog row.
- Small blast radius: one renderer hook + one Rust command call site + tests.
- Parity with autopilot/tailor: same persistence + same harvesting.
- The failure audit found `scrape_resolve_url` returns null for every failure (no distinct rate-limit shape), so the UI shows the generic inline error — a distinct error union is a recorded fast-follow.

## Owning symbols

- `apps/desktop/src/renderer/components/job/JobUrlImport`
- `apps/desktop/src/renderer/features/ai-generate/hooks/useGeneration.ts` (`persist` function)
- `apps/desktop/src-tauri/src/commands/scrape.rs` (`scrape_resolve_url`)
- `apps/desktop/src-tauri/src/commands/discovery.rs` (harvest seam: `posting_to_ref`, `HarvestSource`)
- `packages/shared/src/ipc/contracts` (`AiGenerationSaveRequest` fields: `jobUrl`, `board`)
