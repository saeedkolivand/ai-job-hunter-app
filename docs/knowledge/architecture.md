# Architecture (map + boundaries + feature ownership)

Last updated: 2026-06-02

Canonical: [`docs/ARCHITECTURE.md`](../ARCHITECTURE.md), [`docs/architecture-rules.md`](../architecture-rules.md) (the L0–L3 rules, tested by `cargo test --test architecture`), [`docs/PATTERNS.md`](../PATTERNS.md). Use `graphify explain "<module>"` for a scoped view.

## Shape

Local-first desktop app, [pnpm][pnpm] monorepo. **[Tauri][tauri] is the shell.**

- `packages/shared` — IPC contracts + [Zod][zod] schemas + types (no [React][react], no Node).
- `packages/ui` — [React][react] component library + design system (no app logic, no IPC).
- `packages/prompts` — AI prompt templates, provider-aware + locale-driven (pure [TypeScript][typescript], zero deps).
- `apps/tauri` — [Rust][rust] core (`src-tauri/src/`) + [React][react] renderer (`src/renderer/`).

## Rust/TS boundary (Rust-first)

Business logic, processing pipelines, ATS analysis, and document generation live in **Rust** (`apps/tauri/src-tauri/src/`). The renderer is presentation-only and reaches the shell **only** via the `AppClient` context (`apps/tauri/src/tauri-client.ts` → `createTauriInvokeClient()`). IPC contract source of truth: `packages/shared/src/ipc/` (+ `apps/tauri/src-tauri/src/ipc_contracts/`).

## L0–L3 layers (enforced)

L0 platform/net/error → L1 domain → L2 services/commands → L3 entrypoints. Hard rules (CI-failing): env only in `platform/`; `reqwest::Client` only in `net/`; typed errors via `error.rs` (`AppError`/`AppResult`). See `docs/architecture-rules.md`.

## Backend modules (`apps/tauri/src-tauri/src/`)

- **Resume/export** — `export/` (pdf/, docx/, typst_engine/, model_docx/, templates/, parser/, links/, types.rs), `model/`, `theme/`, `locale/`, `contact_profile/`, `validate/`.
- **Job match / ATS** — `commands/match_resume.rs`, `commands/cover_letter.rs` + `cover_letter/`, `recommend/`, `validate/`.
- **Automation** — `scraping/` (boards/, engine/, http/, linkedin/, board_login/), `applying/` (boards/, registry/, form_filler/, selectors/), `browser/`, `apply_helpers/`, `autopilot/`.
- **AI** — `commands/ai_provider/` (ollama/openai/anthropic/gemini + cli_agent), `commands/ai.rs`, `documents/` (embeddings), `ai_generations/`, `conversations/`, `extraction/`, `recommend/`.
- **Platform/data** — `platform/` (`config.rs` `data_dir()`), `net/` (`http.rs` `shared()`), `error.rs`, `observability.rs` (`Span`), `db.rs`, `data_store.rs`, `credentials/`, `updater/`, `pipeline/`, `jobs/`, `postings/`, `job_preferences/`, `profile_import/`.

## Feature ownership (frontend ↔ domain ↔ agent)

Renderer (`apps/tauri/src/renderer/`): ~14 features each owning a route + service hooks (`renderer/services/`, [TanStack Query][tanstack-query]), [Zustand][zustand] stores, state machines (`lib/machines/`). Map: jobs/search/monitoring → backend jobs/scraping; ai-generate/ai-workspace → AI + resume/export; resumes/resume → resume-export domain; autopilot/onboarding → automation; settings/privacy → platform/security.

| Area                                 | Owner agent                                        | Key paths                                                                      |
| ------------------------------------ | -------------------------------------------------- | ------------------------------------------------------------------------------ |
| Resume / export / templates          | `resume-export-expert` (impl `pdf-docx-generator`) | `export/` (incl. `typst_engine/`), `model/`, `theme/`, `locale/`, `templates/` |
| ATS scoring / job match              | `job-match-expert`                                 | `commands/match_resume.rs`, `cover_letter`, `recommend/`, `validate/`          |
| Scraping / applying                  | `scraping-applier-expert`                          | `scraping/`, `applying/`, `browser/`                                           |
| AI providers / embeddings / prompts  | `ai-provider-expert`                               | `commands/ai_provider/`, `commands/ai.rs`, `documents/`, `packages/prompts`    |
| Rust backend / data / migrations     | `rust-backend-architect`                           | rest of `src-tauri/src/**`, `db.rs`, `*Store`                                  |
| Security (cross-cutting)             | `tauri-security-reviewer`                          | `capabilities/`, `net/`, `credentials/`, deps, `updater/`                      |
| Frontend / UI / a11y / i18n          | `frontend-reviewer`                                | `apps/tauri/src/renderer/**`, `packages/ui`                                    |
| Docs / knowledge / lessons / release | `project-steward`                                  | `docs/`, `docs/knowledge/`, release config                                     |

[tauri]: https://tauri.app
[pnpm]: https://pnpm.io
[react]: https://react.dev
[rust]: https://www.rust-lang.org
[typescript]: https://www.typescriptlang.org
[zod]: https://zod.dev
[tanstack-query]: https://tanstack.com/query
[zustand]: https://github.com/pmndrs/zustand
