# Architecture Status — AI Job Hunter

Implementation status tracker. Updated as features ship.

Last updated: 2026-06-24

---

## Legend

| Symbol | Meaning               |
| ------ | --------------------- |
| ✅     | Fully implemented     |
| 🚧     | In progress / partial |
| ⬜     | Planned / not started |

---

## Infrastructure

| Component                                        | Status | Notes                                                                                                                                                                                             |
| ------------------------------------------------ | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [Tauri][tauri] 2.x shell                         | ✅     | Window, tray, menus, IPC                                                                                                                                                                          |
| [pnpm][pnpm] monorepo + [Turborepo][turborepo]   | ✅     | All packages wired                                                                                                                                                                                |
| [TypeScript][typescript] 6 across all packages   | ✅     | Strict mode enabled                                                                                                                                                                               |
| [Vite][vite] + HMR for renderer                  | ✅     |                                                                                                                                                                                                   |
| [TanStack Router][tanstack-router] (file-based)  | ✅     | All 9 routes                                                                                                                                                                                      |
| [TanStack Query][tanstack-query] + service hooks | ✅     | All 23 namespaces                                                                                                                                                                                 |
| [Zustand][zustand] stores                        | ✅     | preferences-store, generation-store (`store/generation-store/`), others                                                                                                                           |
| AppClient / mock transport                       | ✅     | [Tauri][tauri] + mock implementations                                                                                                                                                             |
| [ESLint][eslint] + [Prettier][prettier]          | ✅     | Enforced in CI                                                                                                                                                                                    |
| [Husky][husky] + [commitlint][commitlint]        | ✅     | Pre-commit hooks                                                                                                                                                                                  |
| Semantic release pipeline                        | ✅     | Auto-versioning on main                                                                                                                                                                           |
| Auto-updater                                     | ✅     | GitHub Releases integration                                                                                                                                                                       |
| Native desktop behaviors                         | ✅     | `apps/desktop/src/desktop-native.ts` (`installDesktopNativeBehaviors`): context-menu, zoom, reload guards (prod-only); selection opt-in via `.select-text` / `[data-selectable]` in `globals.css` |
| Data backup / restore                            | ✅     | `DataStore` trait → full export/import bundle (Settings → Privacy)                                                                                                                                |
| Full app reset                                   | ✅     | `privacy_reset_app` wipes every store registered in the `Resettable` registry (`commands/privacy.rs`)                                                                                             |
| Shared platform layers                           | ✅     | `platform::config`, `net::http`, `error::AppError`, `observability::Span` + provider/board registries (Phases 1–6 — see PATTERNS.md §13)                                                          |
| Architecture CI guardrails                       | ✅     | grep bans: `std::env::var` outside `platform/config.rs`; `reqwest::Client::new/builder` outside `net/http.rs`; `Result<_, String>` outside `error.rs`                                             |
| PDF engine migration (printpdf → Typst)          | ✅     | `printpdf` + `ttf-parser` removed; `export/layout_pdf.rs`, `export/pdf_renderer/`, top-level `layout/`, `measure/` deleted; `export/typst_engine/` is the sole PDF backend                        |
| Centralized SQLite (`db::open`)                  | ✅     | WAL mode + 5s busy_timeout; routed by all stores; atomic transactions on import/migration/status (ADR-022)                                                                                        |
| Anti-abuse rate + concurrency limits             | ✅     | In-memory `RateLimited` error (H13) on `ai_generate` + scrape commands; per-provider daily ceiling + concurrent-op limit (`limits/` module)                                                       |
| OS accent live-update watcher                    | ✅     | Windows WinRT `UISettings::ColorValuesChanged` → `system:accentChanged` event; renderer re-applies theme when accentSource='system' (macOS deferred)                                              |

---

## AI & Data (Rust core)

AI generation (`commands/ai_provider/`) and data/document handling
(`documents/`, `jobs/`, [SQLite][sqlite] via [rusqlite][rusqlite]) now live in the [Rust][rust] core; the
former `packages/ai` and `packages/data` Node packages were removed.

| Feature                     | Status | Notes                                                                                                 |
| --------------------------- | ------ | ----------------------------------------------------------------------------------------------------- |
| Resume/document storage     | ✅     | SQLite `documents` table, embeddings in `vectors` table                                               |
| Job-posting vector cache    | ✅     | `posting_vectors` table (Phase 1 of faster match scoring); self-invalidating on space/text change     |
| Match-score result cache    | ✅     | `match_scores` table (Phase 1); composite PK encodes formula version, space, semantic flag, text hash |
| ATS keyword matching        | ✅     | Stemmed matching with language detection from job ad                                                  |
| Semantic similarity scoring | ✅     | Embedding-based cosine similarity; local Ollama or cloud providers (OpenAI, Anthropic, Gemini)        |

> ⚠️ This feature matrix previously tracked the deleted Node packages and has
> not been re-audited against the Rust implementation. Some features that were
> ✅ in TypeScript (notably hybrid search) are currently stubs in Rust — verify
> against the code before relying on this.

---

## Scraping (`apps/desktop/src-tauri/src/scraping/`)

Active scrapers: 20 boards. Five boards (Indeed, StepStone, Xing, Workday, Glassdoor) were retired as direct scrapers in 2026-06-21 and are now covered by the Aggregator (Adzuna/JSearch). See ADR-026.

| Board                           | Status  | Notes                                                              |
| ------------------------------- | ------- | ------------------------------------------------------------------ |
| Aggregator (Adzuna/JSearch)     | ✅      | Bring-your-own-key; covers Indeed/StepStone/Xing/Workday/Glassdoor |
| LinkedIn                        | ✅      | Session cookie required for higher rate limits                     |
| Greenhouse                      | ✅      | ATS platform; company-scoped                                       |
| Lever                           | ✅      | ATS platform; company-scoped                                       |
| Ashby                           | ✅      | ATS platform; company-scoped                                       |
| SmartRecruiters                 | ✅      | Company-scoped                                                     |
| Recruitee                       | ✅      | Company-scoped                                                     |
| Personio                        | ✅      | Company-scoped                                                     |
| BambooHR                        | ✅      | Company-scoped                                                     |
| Breezy HR                       | ✅      | Company-scoped                                                     |
| Pinpoint                        | ✅      | Company-scoped                                                     |
| Rippling                        | ✅      | Company-scoped                                                     |
| RemoteOK                        | ✅      |                                                                    |
| Remotive                        | ✅      |                                                                    |
| Arbeitsagentur                  | ✅      | German federal job agency                                          |
| BerlinStartupJobs               | ✅      |                                                                    |
| GermanTechJobs                  | ✅      |                                                                    |
| ArbeitNow                       | ✅      |                                                                    |
| YCombinator (Work at a Startup) | ✅      |                                                                    |
| We Work Remotely                | ✅      | RSS feed                                                           |
| Indeed                          | Retired | Anti-bot walls; covered via Aggregator (ADR-026)                   |
| StepStone                       | Retired | Anti-bot walls; covered via Aggregator (ADR-026)                   |
| Xing                            | Retired | Anti-bot walls; covered via Aggregator (ADR-026)                   |
| Workday                         | Retired | Anti-bot walls; covered via Aggregator (ADR-026)                   |
| Glassdoor                       | Retired | Anti-bot walls; covered via Aggregator (ADR-026)                   |

---

## AI Generation (`apps/desktop/src/renderer/features/ai-generate/`)

| Feature                    | Status | Notes                                                                                                                     |
| -------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------- |
| Cover letter generation    | ✅     | Streaming                                                                                                                 |
| Resume generation          | ✅     | Streaming                                                                                                                 |
| Email generation           | ✅     |                                                                                                                           |
| Summary generation         | ✅     |                                                                                                                           |
| Bold keyword extraction    | ✅     | Post-processes output                                                                                                     |
| DOCX export                | ✅     | `export/docx/` + `model_docx.rs` (docx-rs): real two-column table + native ATS; A4 + font fallback                        |
| PDF export                 | ✅     | Typst engine (`export/typst_engine/`); Carlito + Noto Sans vendored via `include_bytes!`; CJK deferred (tofu + UI notice) |
| ATS-safe linearization     | ✅     | Two-column → single for ATS                                                                                               |
| Universal thinking display | ✅     | All providers normalized via `think-split.ts`; `ThinkingBubble` UI (`ai-generate/components/`)                            |
| Local model limits         | ✅     | `ai_inspect_model` IPC; `modelLimits` in preferences-store; `num_ctx`/`num_predict` on [Ollama][ollama] path only         |
| Company research           | ✅     | `ai_research_company` IPC; opt-in; active provider's own web search (native tool / Ollama Web Search); untrusted-fenced   |
| Application questions      | ✅     | `APPLICATION_QUESTIONS` registry + grounded answer prompt; answers persist on per-job record                              |
| Locale-aware prompts       | ✅     | 11 languages                                                                                                              |
| Template preview           | ✅     | OptionTile with live preview                                                                                              |

---

## Resume Analysis (`apps/desktop/src/renderer/features/analyze/`)

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

## Autopilot (`apps/desktop/src-tauri/src/autopilot/`)

| Feature                      | Status | Notes                                                                                                                                                                            |
| ---------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Workflow definition wizard   | ✅     | 3-step UI                                                                                                                                                                        |
| Workflow persistence         | ✅     | [SQLite][sqlite]                                                                                                                                                                 |
| Manual trigger               | ✅     |                                                                                                                                                                                  |
| Scheduled execution          | ✅     | Cron-like scheduler                                                                                                                                                              |
| Real-time step events        | ✅     | autopilot:step stream                                                                                                                                                            |
| Pause / resume               | ✅     |                                                                                                                                                                                  |
| Found-job dedup + tracking   | ✅     | `merge_found_jobs` dedup by URL; `FoundJob.is_new`; `applied` derived from `ai_generations.job_url`                                                                              |
| Generation-session store     | ✅     | `store/generation-store/` — app-wide, keyed by context id, survives navigation; Tailor modal uses it                                                                             |
| `ai_generations` aggregate   | ✅     | `job_url`, `board`, `application_answers`, `company_brief` columns; per-job merge-upsert (`merge_application`)                                                                   |
| `run_status` + status badge  | ✅     | `inProgress\|completed\|failed\|interrupted`; amber/red chip on `AutopilotCard`; crash reconciliation on startup                                                                 |
| OS notification on new jobs  | ✅     | Permission-gated; clicking the notification navigates to `/autopilot`                                                                                                            |
| Tray module                  | ✅     | Dynamic "New jobs: N" click→focus; "Pause all" — `apps/desktop/src-tauri/src/tray/`                                                                                              |
| Deep-link focus guard        | ✅     | `ajh://autopilot/<id>` validated against strict allowlist; registered OS scheme — `deeplink/`                                                                                    |
| Startup catch-up sweep       | ✅     | Fires ~5 s after launch instead of waiting a full tick interval                                                                                                                  |
| `minMatchScore` enforcement  | ✅     | Scorable postings below threshold dropped before `record_run`; unscored postings kept                                                                                            |
| Cancellation token reuse     | ✅     | Tray/UI cancel reaches the running token across the whole run                                                                                                                    |
| Launch-at-login              | ✅     | Opt-in (default OFF); `system_get/set_launch_at_login` via `tauri-plugin-autostart`                                                                                              |
| Ranking via keyword-coverage | ✅     | Unified on `documents::keywords::coverage_score` (embedding-free, pure keyword stemming + matching); relabeled "Keyword Coverage %" to distinguish from Jobs "Match %" (ADR-020) |

---

## UI / UX

| Feature                   | Status | Notes                                                                                                                                                                                           |
| ------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Dashboard route           | ✅     | Pipeline overview, recent activity                                                                                                                                                              |
| Jobs route                | ✅     | Master-detail split view (default) + list-only toggle; virtualized list; filter, interaction history, viewed/applied badges. See `apps/desktop/src/renderer/features/jobs/` for layout details. |
| Search route              | ✅     | Hybrid semantic search (⌘/Ctrl+K only — removed from sidebar)                                                                                                                                   |
| AI route                  | ✅     | Model selection, [Ollama][ollama] health                                                                                                                                                        |
| AI Generate route         | ✅     | Full generation UI                                                                                                                                                                              |
| Analyze route             | ✅     | Resume analysis panels                                                                                                                                                                          |
| Autopilot route           | ✅     | Workflow builder + runner                                                                                                                                                                       |
| Documents route           | ✅     | Three-tab view — Résumés / Cover Letters / Activity (lenses over `ai_generations`); route stays `/resumes`                                                                                      |
| Settings route            | ✅     | All settings tabs; keyboard-reachable sidebar (`@ajh/ui Button` + `aria-current`); `SettingsSection` throughout                                                                                 |
| Support route             | ✅     | Diagnostics, FAQ, logs                                                                                                                                                                          |
| Onboarding wizard         | ✅     | First-run experience                                                                                                                                                                            |
| Light/dark/system theme   | ✅     |                                                                                                                                                                                                 |
| i18n (11 languages)       | ✅     | UI translations                                                                                                                                                                                 |
| Keyboard shortcuts        | ✅     | Global handler + `?` cheat-sheet modal (`useKeyboardShortcuts`)                                                                                                                                 |
| Auto-updater banner       | ✅     |                                                                                                                                                                                                 |
| Performance mode selector | ✅     |                                                                                                                                                                                                 |
| Spotlight tour            | ✅     | Interactive tutorial                                                                                                                                                                            |
| Sidebar nav groups        | ✅     | Workspace / Automation / pinned — `nav.sections.workspace\|automation` i18n keys                                                                                                                |
| Grouped nav pill          | ✅     | `@ajh/ui NavPill` slides within each list (`layoutId` scoped per group)                                                                                                                         |
| `SegmentedControl`        | ✅     | `@ajh/ui` — radiogroup + roving arrow-key nav; `track`/`grid` variants                                                                                                                          |
| `SetupHint`               | ✅     | `@ajh/ui` — generalized contextual setup nudge (AI + future board/chrome)                                                                                                                       |
| Visible focus rings       | ✅     | Global `:focus-visible` ring; `ModalShell` `aria-labelledby`; `role="switch"` toggles                                                                                                           |
| Optimistic delete         | ✅     | `onMutate` snapshot+filter / `onError` rollback on generations + autopilots                                                                                                                     |
| macOS window vibrancy     | ⬜     | Deferred — requires a Mac-capable dev session (`window-vibrancy` crate)                                                                                                                         |

---

## Planned / Backlog

| Feature                                 | Priority | Notes                                                                                                                                                             |
| --------------------------------------- | -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| CJK font support in PDF/preview         | Medium   | Bundle Noto Sans CJK into Typst engine (`export/typst_engine/`) so zh/ja/ko render in PDF and live preview (generation + DOCX already work; currently shows tofu) |
| URL-to-job-ad extraction in AI Generate | Medium   | `scrape.url` IPC contract exists; UI input not yet wired                                                                                                          |
| LinkedIn official API integration       | Medium   | Currently Playwright-only                                                                                                                                         |
| Advanced skill taxonomy                 | Medium   | Structured ontology for matching                                                                                                                                  |
| Salary negotiation assistant            | Low      |                                                                                                                                                                   |
| Cloud sync                              | Low      | Deferred — needs a remote backend; the backup bundle + `DataStore` trait are the substrate                                                                        |
| Team/shared job tracking                | Low      | Would require cloud sync                                                                                                                                          |
| Interview preparation AI                | Medium   | Mock interview Q&A                                                                                                                                                |

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
