# Architecture Status — AI Job Hunter

Implementation status tracker. Updated as features ship.

Last updated: 2026-06-01

---

## Legend

| Symbol | Meaning               |
| ------ | --------------------- |
| ✅     | Fully implemented     |
| 🚧     | In progress / partial |
| ⬜     | Planned / not started |

---

## Infrastructure

| Component                                        | Status | Notes                                                                                                                                                                                           |
| ------------------------------------------------ | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [Tauri][tauri] 2.x shell                         | ✅     | Window, tray, menus, IPC                                                                                                                                                                        |
| [pnpm][pnpm] monorepo + [Turborepo][turborepo]   | ✅     | All packages wired                                                                                                                                                                              |
| [TypeScript][typescript] 6 across all packages   | ✅     | Strict mode enabled                                                                                                                                                                             |
| [Vite][vite] + HMR for renderer                  | ✅     |                                                                                                                                                                                                 |
| [TanStack Router][tanstack-router] (file-based)  | ✅     | All 9 routes                                                                                                                                                                                    |
| [TanStack Query][tanstack-query] + service hooks | ✅     | All 21 namespaces                                                                                                                                                                               |
| [Zustand][zustand] stores                        | ✅     | preferences-store, generation-store (`store/generation-store/`), others                                                                                                                         |
| AppClient / mock transport                       | ✅     | [Tauri][tauri] + mock implementations                                                                                                                                                           |
| [ESLint][eslint] + [Prettier][prettier]          | ✅     | Enforced in CI                                                                                                                                                                                  |
| [Husky][husky] + [commitlint][commitlint]        | ✅     | Pre-commit hooks                                                                                                                                                                                |
| Semantic release pipeline                        | ✅     | Auto-versioning on main                                                                                                                                                                         |
| Auto-updater                                     | ✅     | GitHub Releases integration                                                                                                                                                                     |
| Native desktop behaviors                         | ✅     | `apps/tauri/src/desktop-native.ts` (`installDesktopNativeBehaviors`): context-menu, zoom, reload guards (prod-only); selection opt-in via `.select-text` / `[data-selectable]` in `globals.css` |
| Data backup / restore                            | ✅     | `DataStore` trait → full export/import bundle (Settings → Privacy)                                                                                                                              |
| Full app reset                                   | ✅     | `privacy_reset_app` wipes every store registered in the `Resettable` registry (`commands/privacy.rs`)                                                                                           |
| Shared platform layers                           | ✅     | `platform::config`, `net::http`, `error::AppError`, `observability::Span` + provider/board registries (Phases 1–6 — see PATTERNS.md §13)                                                        |
| Architecture CI guardrails                       | ✅     | grep bans: `std::env::var` outside `platform/config.rs`; `reqwest::Client::new/builder` outside `net/http.rs`; `Result<_, String>` outside `error.rs`                                           |

---

## AI & Data (Rust core)

AI generation (`commands/ai_provider/`) and data/document handling
(`documents/`, `jobs/`, [SQLite][sqlite] via [rusqlite][rusqlite]) now live in the [Rust][rust] core; the
former `packages/ai` and `packages/data` Node packages were removed.

> ⚠️ This feature matrix previously tracked the deleted Node packages and has
> not been re-audited against the Rust implementation. Some features that were
> ✅ in TypeScript (notably hybrid search and the resume-job matcher) are
> currently stubs in Rust — verify against the code before relying on this.

---

## Scraping (`apps/tauri/src-tauri/src/scraping/`)

| Board                           | Status | Notes                                      |
| ------------------------------- | ------ | ------------------------------------------ |
| LinkedIn                        | ✅     | Playwright; session cookie required        |
| Indeed                          | ✅     |                                            |
| StepStone                       | ✅     |                                            |
| Xing                            | ✅     | DACH market                                |
| Greenhouse                      | ✅     | ATS platform                               |
| Lever                           | ✅     | ATS platform                               |
| Ashby                           | ✅     | ATS platform                               |
| SmartRecruiters                 | ✅     |                                            |
| Recruitee                       | ✅     |                                            |
| Personio                        | ✅     |                                            |
| Workday                         | ✅     |                                            |
| RemoteOK                        | ✅     |                                            |
| Remotive                        | ✅     |                                            |
| Arbeitsagentur                  | ✅     | German federal job agency                  |
| BerlinStartupJobs               | ✅     |                                            |
| GermanTechJobs                  | ✅     |                                            |
| ArbeitNow                       | ✅     |                                            |
| YCombinator (Work at a Startup) | ✅     |                                            |
| Captcha detection + handling    | ✅     | Reports to UI, waits for manual resolution |

---

## AI Generation (`apps/tauri/src/renderer/features/ai-generate/`)

| Feature                    | Status | Notes                                                                                                                   |
| -------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------------- |
| Cover letter generation    | ✅     | Streaming                                                                                                               |
| Resume generation          | ✅     | Streaming                                                                                                               |
| Email generation           | ✅     |                                                                                                                         |
| Summary generation         | ✅     |                                                                                                                         |
| Bold keyword extraction    | ✅     | Post-processes output                                                                                                   |
| DOCX export                | ✅     | Canonical model engine (default): real two-column table + native ATS; A4 + font fallback; legacy fallback               |
| PDF export                 | ✅     | Canonical layout engine; glyph-subset fonts (`pdf_renderer/fonts.rs`) ~120 KB vs ~3 MB full embed                       |
| ATS-safe linearization     | ✅     | Two-column → single for ATS                                                                                             |
| Universal thinking display | ✅     | All providers normalized via `think-split.ts`; `ThinkingBubble` UI (`ai-generate/components/`)                          |
| Local model limits         | ✅     | `ai_inspect_model` IPC; `modelLimits` in preferences-store; `num_ctx`/`num_predict` on [Ollama][ollama] path only       |
| Company research           | ✅     | `ai_research_company` IPC; opt-in; active provider's own web search (native tool / Ollama Web Search); untrusted-fenced |
| Application questions      | ✅     | `APPLICATION_QUESTIONS` registry + grounded answer prompt; answers persist on per-job record                            |
| Locale-aware prompts       | ✅     | 11 languages                                                                                                            |
| Template preview           | ✅     | OptionTile with live preview                                                                                            |

---

## Resume Analysis (`apps/tauri/src/renderer/features/analyze/`)

| Feature                    | Status | Notes |
| -------------------------- | ------ | ----- |
| ATS score                  | ✅     |       |
| Skill extraction           | ✅     |       |
| Skill gap detection        | ✅     |       |
| Language mismatch warning  | ✅     |       |
| Strength/weakness analysis | ✅     |       |
| Recommendations            | ✅     |       |
| Experience level detection | ✅     |       |

---

## Autopilot (`apps/tauri/src-tauri/src/autopilot/`)

| Feature                      | Status | Notes                                                                                                          |
| ---------------------------- | ------ | -------------------------------------------------------------------------------------------------------------- |
| Workflow definition wizard   | ✅     | 3-step UI                                                                                                      |
| Workflow persistence         | ✅     | [SQLite][sqlite]                                                                                               |
| Manual trigger               | ✅     |                                                                                                                |
| Scheduled execution          | ✅     | Cron-like scheduler                                                                                            |
| Real-time step events        | ✅     | autopilot:step stream                                                                                          |
| Pause / resume               | ✅     |                                                                                                                |
| Found-job dedup + tracking   | ✅     | `merge_found_jobs` dedup by URL; `FoundJob.is_new`; `applied` derived from `ai_generations.job_url`            |
| Generation-session store     | ✅     | `store/generation-store/` — app-wide, keyed by context id, survives navigation; Apply modal uses it            |
| `ai_generations` aggregate   | ✅     | `job_url`, `board`, `application_answers`, `company_brief` columns; per-job merge-upsert (`merge_application`) |
| Applications/History view    | ✅     | Generated tab — new/applied badges; per-job record in history card                                             |
| Auto-apply integration       | 🚧     | Apply success rate varies by board                                                                             |
| Batch application throttling | 🚧     | Rate limiting per board                                                                                        |

---

## UI / UX

| Feature                   | Status | Notes                                    |
| ------------------------- | ------ | ---------------------------------------- |
| Dashboard route           | ✅     | Pipeline overview, recent activity       |
| Jobs route                | ✅     | List, filter, interaction history        |
| Search route              | ✅     | Hybrid semantic search                   |
| AI route                  | ✅     | Model selection, [Ollama][ollama] health |
| AI Generate route         | ✅     | Full generation UI                       |
| Analyze route             | ✅     | Resume analysis panels                   |
| Autopilot route           | ✅     | Workflow builder + runner                |
| Settings route            | ✅     | All settings tabs                        |
| Support route             | ✅     | Diagnostics, FAQ, logs                   |
| Onboarding wizard         | ✅     | First-run experience                     |
| Light/dark theme          | ✅     |                                          |
| i18n (11 languages)       | ✅     | UI translations                          |
| Keyboard shortcuts        | ✅     | Configurable                             |
| Auto-updater banner       | ✅     |                                          |
| Performance mode selector | ✅     |                                          |
| Spotlight tour            | ✅     | Interactive tutorial                     |

---

## Planned / Backlog

| Feature                                 | Priority | Notes                                                                                      |
| --------------------------------------- | -------- | ------------------------------------------------------------------------------------------ |
| URL-to-job-ad extraction in AI Generate | Medium   | `scrape.url` IPC contract exists; UI input not yet wired                                   |
| LinkedIn official API integration       | Medium   | Currently Playwright-only                                                                  |
| Browser extension (quick apply)         | Low      |                                                                                            |
| Advanced skill taxonomy                 | Medium   | Structured ontology for matching                                                           |
| Salary negotiation assistant            | Low      |                                                                                            |
| Cloud sync                              | Low      | Deferred — needs a remote backend; the backup bundle + `DataStore` trait are the substrate |
| Team/shared job tracking                | Low      | Would require cloud sync                                                                   |
| Interview preparation AI                | Medium   | Mock interview Q&A                                                                         |

[tauri]: https://tauri.app
[pnpm]: https://pnpm.io
[turborepo]: https://turborepo.com
[typescript]: https://www.typescriptlang.org
[vite]: https://vite.dev
[tanstack-router]: https://tanstack.com/router
[tanstack-query]: https://tanstack.com/query
[zustand]: https://github.com/pmndrs/zustand
[eslint]: https://eslint.org
[prettier]: https://prettier.io
[husky]: https://typicode.github.io/husky
[commitlint]: https://commitlint.js.org
[sqlite]: https://www.sqlite.org
[rusqlite]: https://github.com/rusqlite/rusqlite
[rust]: https://www.rust-lang.org
[ollama]: https://ollama.com
