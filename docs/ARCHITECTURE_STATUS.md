# Architecture Status — AI Job Hunter

Implementation status tracker. Updated as features ship.

Last updated: 2026-05-28

---

## Legend

| Symbol | Meaning               |
| ------ | --------------------- |
| ✅     | Fully implemented     |
| 🚧     | In progress / partial |
| ⬜     | Planned / not started |

---

## Infrastructure

| Component                        | Status | Notes                                                              |
| -------------------------------- | ------ | ------------------------------------------------------------------ |
| Tauri 2.x shell                  | ✅     | Window, tray, menus, IPC                                           |
| pnpm monorepo + Turbo            | ✅     | All packages wired                                                 |
| TypeScript 6 across all packages | ✅     | Strict mode enabled                                                |
| Vite + HMR for renderer          | ✅     |                                                                    |
| TanStack Router (file-based)     | ✅     | All 9 routes                                                       |
| TanStack Query + service hooks   | ✅     | All 21 namespaces                                                  |
| Zustand stores                   | ✅     | preferences-store, others                                          |
| AppClient / mock transport       | ✅     | Tauri + mock implementations                                       |
| ESLint + Prettier                | ✅     | Enforced in CI                                                     |
| Husky + commitlint               | ✅     | Pre-commit hooks                                                   |
| Semantic release pipeline        | ✅     | Auto-versioning on main                                            |
| Auto-updater                     | ✅     | GitHub Releases integration                                        |
| Data backup / restore            | ✅     | `DataStore` trait → full export/import bundle (Settings → Privacy) |

---

## AI & Data (Rust core)

AI generation (`commands/ai_provider.rs`) and data/document handling
(`documents/`, `jobs/`, SQLite via `rusqlite`) now live in the Rust core; the
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

| Feature                       | Status | Notes                        |
| ----------------------------- | ------ | ---------------------------- |
| Cover letter generation       | ✅     | Streaming                    |
| Resume generation             | ✅     | Streaming                    |
| Email generation              | ✅     |                              |
| Summary generation            | ✅     |                              |
| Bold keyword extraction       | ✅     | Post-processes output        |
| DOCX export                   | ✅     | All 9 templates              |
| PDF export                    | ✅     |                              |
| ATS-safe linearization        | ✅     | Two-column → single for ATS  |
| Extended thinking (Anthropic) | ✅     | ThinkingBubble UI            |
| Locale-aware prompts          | ✅     | 11 languages                 |
| Template preview              | ✅     | OptionTile with live preview |

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

| Feature                      | Status | Notes                              |
| ---------------------------- | ------ | ---------------------------------- |
| Workflow definition wizard   | ✅     | 3-step UI                          |
| Workflow persistence         | ✅     | SQLite                             |
| Manual trigger               | ✅     |                                    |
| Scheduled execution          | ✅     | Cron-like scheduler                |
| Real-time step events        | ✅     | autopilot:step stream              |
| Pause / resume               | ✅     |                                    |
| Auto-apply integration       | 🚧     | Apply success rate varies by board |
| Batch application throttling | 🚧     | Rate limiting per board            |

---

## UI / UX

| Feature                   | Status | Notes                              |
| ------------------------- | ------ | ---------------------------------- |
| Dashboard route           | ✅     | Pipeline overview, recent activity |
| Jobs route                | ✅     | List, filter, interaction history  |
| Search route              | ✅     | Hybrid semantic search             |
| AI route                  | ✅     | Model selection, Ollama health     |
| AI Generate route         | ✅     | Full generation UI                 |
| Analyze route             | ✅     | Resume analysis panels             |
| Autopilot route           | ✅     | Workflow builder + runner          |
| Settings route            | ✅     | All settings tabs                  |
| Support route             | ✅     | Diagnostics, FAQ, logs             |
| Onboarding wizard         | ✅     | First-run experience               |
| Light/dark theme          | ✅     |                                    |
| i18n (11 languages)       | ✅     | UI translations                    |
| Keyboard shortcuts        | ✅     | Configurable                       |
| Auto-updater banner       | ✅     |                                    |
| Performance mode selector | ✅     |                                    |
| Spotlight tour            | ✅     | Interactive tutorial               |

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
| Application analytics dashboard         | Medium   | Track apply→response rates                                                                 |
