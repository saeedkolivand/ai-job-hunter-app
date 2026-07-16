# ADR-013: Resume Builder — Job-Agnostic Base + In-Memory Tailoring Handoff

## Status

Accepted (merged in PR #326)

## Context

The user requested a way to build a new résumé from scratch without starting from an existing document. The previous flow required importing or selecting an existing document first, which created friction for brand-new users.

We needed to provide a **frictionless on-ramp** for résumé creation while:

1. Reusing the existing export/preview/link pipeline unchanged
2. Avoiding new IPC boundaries or persistent storage for intermediate state
3. Keeping the "tailor to a job" flow optional and in-memory

## Decision

Build a **dedicated résumé-builder UI** that:

- **Creates a job-agnostic base résumé** via a wizard (interview-questions → `synthesizeResume()` → markdown)
- **Emits the same markdown grammar** as the AI-Generate flow (same headers, structure, localized keys via `@ajh/prompts/builder`)
- **Avoids new IPC**: built content passes in-memory to the export/tailoring pipeline
- **Hands off to AI-Generate for tailoring**: the base résumé can be selected in the "Tailor to a job" flow
- **Uses existing persistence**: once generated, the user exports it (`documents.import`) or saves it via `aiGenerations.save` (no new DocumentRecord store)

## Consequences

### Positive

- **Zero data-store friction**: Interview answers are session-only; no new database schema
- **Reusable output**: The synthesized markdown works with every downstream subsystem (export, preview, link, tailoring, storage)
- **Graceful onboarding**: New users can build a base résumé without needing an import step
- **Prompt parity**: System prompts in `@ajh/prompts/builder` mirror the AI-Generate contracts (no-fabrication clause, same locale-driven headers)

### Tradeoffs

- **Session-only state**: If the user exits without saving, the built résumé is lost (same as in-app generation)
- **Manual export required**: Unlike a scrape workflow, there's no automatic persistence; the user must choose to export or save

## Implementation Details

- **New prompts module**: `packages/prompts/src/builder/` exports `buildBuilderSystemPrompt`, `buildInterviewResumePrompt`, `renderInterviewAnswers`, `InterviewAnswers` type
- **New renderer flow**: `features/resume-builder/**` (wizard, RepeatableList, useResumeBuilder hook)
- **New generation method**: `lib/generate/generation/generation.ts::synthesizeResume()` — mirrors `generateResume()` with a single streamed pass
- **Session store slice**: `store/session-store::resumeBuilder` holds interview state
- **Route**: `routes/build.tsx` → `ROUTES.BUILD` with Sidebar "Resume Builder" nav entry
- **Output panel**: Reuses existing `OutputPanelDone` + `saveAiGeneration` for persistence

See:

- `@ajh/prompts/builder` — prompt templates
- `features/resume-builder` — UI wizard
- `apps/desktop/src/renderer/lib/generate/generation/generation.ts::synthesizeResume()` — generation orchestration
