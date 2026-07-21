# Architecture Status — AI Job Hunter

Implementation status tracker. Updated as features ship.

Last updated: 2026-07-21 (audit refresh: shipped features moved, missing sections added, TypeScript version corrected)

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
| [TypeScript][typescript] 7 (per-importer split)  | ✅     | Apps/desktop/extension use ^7.0.2 (build), root at ^6.0.3 (eslint only). pnpm per-importer resolution lets builds use TS7 while lint stack (typescript-eslint 8.x, capped at <6.1 peer) uses TS6  |
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

---

## Scraping (`apps/desktop/src-tauri/src/scraping/`)

Active scrapers: 21 boards. Five boards (Indeed, StepStone, Xing, Workday, Glassdoor) were retired as direct scrapers in 2026-06-21 and are now covered by the Aggregator (Adzuna/JSearch). See ADR-026.

| Board                              | Status  | Notes                                                                                                                                 |
| ---------------------------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| Aggregator (Adzuna/JSearch/Jooble) | ✅      | Bring-your-own-key; Jooble is third-tier fallback (~67 countries); covers Indeed/StepStone/Xing/Workday/Glassdoor                     |
| LinkedIn                           | ✅      | Session cookie required for higher rate limits                                                                                        |
| Greenhouse                         | ✅      | ATS platform; company-scoped                                                                                                          |
| Lever                              | ✅      | ATS platform; company-scoped                                                                                                          |
| Ashby                              | ✅      | ATS platform; company-scoped                                                                                                          |
| SmartRecruiters                    | ✅      | Company-scoped                                                                                                                        |
| Recruitee                          | ✅      | Company-scoped                                                                                                                        |
| Personio                           | ✅      | Company-scoped                                                                                                                        |
| BambooHR                           | ✅      | Company-scoped                                                                                                                        |
| Breezy HR                          | ✅      | Company-scoped                                                                                                                        |
| Pinpoint                           | ✅      | Company-scoped                                                                                                                        |
| Rippling                           | ✅      | Company-scoped                                                                                                                        |
| RemoteOK                           | ✅      |                                                                                                                                       |
| Remotive                           | ✅      |                                                                                                                                       |
| Arbeitsagentur                     | ✅      | German federal job agency                                                                                                             |
| BerlinStartupJobs                  | ✅      |                                                                                                                                       |
| GermanTechJobs                     | ✅      |                                                                                                                                       |
| ArbeitNow                          | ✅      |                                                                                                                                       |
| The Muse                           | ✅      | Keyword aggregator; no server-side search, client-side filter                                                                         |
| YCombinator (Work at a Startup)    | ✅      |                                                                                                                                       |
| We Work Remotely                   | ✅      | RSS feed                                                                                                                              |
| Indeed                             | Retired | Anti-bot walls; covered via Aggregator (ADR-026)                                                                                      |
| StepStone                          | Retired | Anti-bot walls; covered via Aggregator (ADR-026)                                                                                      |
| Xing                               | Retired | Anti-bot walls; covered via Aggregator (ADR-026)                                                                                      |
| Workday                            | Retired | Anti-bot walls; covered via Aggregator (ADR-026)                                                                                      |
| Glassdoor                          | Retired | Anti-bot walls; covered via Aggregator (ADR-026)                                                                                      |
| Cross-board clustering             | ✅      | Fuzzy recompute-at-ingest, pair tombstones in `dedup.db`, canonical member selection, source chips, "not a duplicate" split (ADR-029) |

---

## AI Generation (`apps/desktop/src/renderer/features/ai-generate/`)

| Feature                    | Status | Notes                                                                                                                                                                                 |
| -------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Cover letter generation    | ✅     | Streaming                                                                                                                                                                             |
| Resume generation          | ✅     | Streaming                                                                                                                                                                             |
| Email generation           | ✅     |                                                                                                                                                                                       |
| Summary generation         | ✅     |                                                                                                                                                                                       |
| Bold keyword extraction    | ✅     | Post-processes output                                                                                                                                                                 |
| DOCX export                | ✅     | `export/docx/` + `model_docx.rs` (docx-rs): real two-column table + native ATS; A4 + font fallback                                                                                    |
| PDF export                 | ✅     | Typst engine (`export/typst_engine/`); Carlito + Inter + Source Serif 4 + Manrope vendored via `include_bytes!`; sets PDF Title/author/lang metadata; CJK deferred (tofu + UI notice) |
| ATS-safe linearization     | ✅     | Two-column → single for ATS                                                                                                                                                           |
| Universal thinking display | ✅     | All providers normalized via `think-split.ts`; `ThinkingBubble` UI (`ai-generate/components/`)                                                                                        |
| Local model limits         | ✅     | `ai_inspect_model` IPC; `modelLimits` in preferences-store; `num_ctx`/`num_predict` on [Ollama][ollama] path only                                                                     |
| Company research           | ✅     | `ai_research_company` IPC; opt-in; active provider's own web search (native tool / Ollama Web Search); untrusted-fenced                                                               |
| Application questions      | ✅     | `APPLICATION_QUESTIONS` registry + grounded answer prompt; answers persist on per-job record                                                                                          |
| Salary expectation helper  | ✅     | Paste-ready number from user expectation + market research (PRs #548, #549); grounds currency in job's country (ADR-0015)                                                             |
| Locale-aware prompts       | ✅     | 11 languages                                                                                                                                                                          |
| Humanized generation tone  | ✅     | Natural-voice LEXICAL/PROSE tiers + Output Tone wiring to escape adversarial AI-detection (PR #563)                                                                                   |
| Template preview           | ✅     | OptionTile with live preview                                                                                                                                                          |
| 12-template gallery        | ✅     | Multi-tier resume templates with Document accent + 3 letter layouts (PRs #590-#594)                                                                                                   |

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

| Feature                      | Status | Notes                                                                                                                                                                                                       |
| ---------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Workflow definition wizard   | ✅     | 3-step UI                                                                                                                                                                                                   |
| Workflow persistence         | ✅     | [SQLite][sqlite]                                                                                                                                                                                            |
| Manual trigger               | ✅     |                                                                                                                                                                                                             |
| Scheduled execution          | ✅     | Cron-like scheduler                                                                                                                                                                                         |
| Real-time step events        | ✅     | autopilot:step stream                                                                                                                                                                                       |
| Pause / resume               | ✅     |                                                                                                                                                                                                             |
| Found-job dedup + tracking   | ✅     | `merge_found_jobs` dedup by URL; cluster-aware dedup via canonical key (ADR-029); `FoundJob.is_new`; new-cluster notification count; `applied` derived from `ai_generations.job_url`                        |
| Generation-session store     | ✅     | `store/generation-store/` — app-wide, keyed by context id, survives navigation; Tailor modal uses it                                                                                                        |
| `ai_generations` aggregate   | ✅     | `job_url`, `board`, `application_answers`, `company_brief` columns; per-job merge-upsert (`merge_application`)                                                                                              |
| `run_status` + status badge  | ✅     | `inProgress\|completed\|failed\|interrupted`; amber/red chip on `AutopilotCard`; crash reconciliation on startup                                                                                            |
| OS notification on new jobs  | ✅     | Permission-gated; clicking the notification navigates to `/autopilot`                                                                                                                                       |
| Tray module                  | ✅     | Dynamic "New jobs: N" click→focus; "Pause all" — `apps/desktop/src-tauri/src/tray/`                                                                                                                         |
| Deep-link focus guard        | ✅     | `ajh://autopilot/<id>` validated against strict allowlist; registered OS scheme — `deeplink/`                                                                                                               |
| Startup catch-up sweep       | ✅     | Fires ~5 s after launch instead of waiting a full tick interval                                                                                                                                             |
| `minMatchScore` enforcement  | ✅     | Cluster-aware: passes iff best member passes; all members of passing cluster kept. Fully-unscored clusters retain old behavior. Mixed clusters with below-bar scored representative dropped whole (ADR-029) |
| Cancellation token reuse     | ✅     | Tray/UI cancel reaches the running token across the whole run                                                                                                                                               |
| Launch-at-login              | ✅     | Opt-in (default OFF); `system_get/set_launch_at_login` via `tauri-plugin-autostart`                                                                                                                         |
| Ranking via keyword-coverage | ✅     | Unified on `documents::keywords::coverage_score` (embedding-free, pure keyword stemming + matching); relabeled "Keyword Coverage %" to distinguish from Jobs "Match %" (ADR-020)                            |

---

## Agentic Features (`apps/desktop/src-tauri/src/agent/`)

Five-step IPC agentic loop: `agent_run` command → validated request → spawned task → `agent:step` event stream → job lifecycle event.

| Feature                          | Status | Notes                                                                                                                                                                                                                                                                                                                                                            |
| -------------------------------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Phase 1: Agent loop controller   | ✅     | Core loop, turn-by-turn control flow, streaming, tool routing. `run_agent_live`, `AgentStepKind`, `StoppedReason` (PR #552)                                                                                                                                                                                                                                      |
| Phase 2: "Prep application" flow | ✅     | 4 read-only tools (research_company, match_resume, draft_cover_letter, suggest_interview_questions); fixed system prompt; trusted ToolContext; no Write tools; display-only proposal (PR #555)                                                                                                                                                                   |
| Phase 3: Proposal confirm gate   | ✅     | Write actions suspend for explicit user approval (approve/edit/deny); `agent_confirm` IPC; 300s timeout; fail-closed (edit+deny default to no-action); validate once on route, once on approval                                                                                                                                                                  |
| Phase 4: Autopilot AI notes      | ✅     | Opt-in `assistant` flag on Autopilot record; generates short LLM notes (why a top match fits, tailoring tip) for top ~3 NEW matches per run; HEADLESS read-only; notify-only; 45s step timeout, daily-ceiling short-circuit, ≤3 calls/run bounded; resolves active provider from backend store (ADR-0012); notes surface on found-job rows + notification suffix |
| Tool-calling model requirement   | ✅     | Validated server-side; non-tool models rejected with clear message (HIGH-2 defense-in-depth)                                                                                                                                                                                                                                                                     |
| Cancellation-token registry      | ✅     | Jobs spawned via `agent_run` register CancellationToken in `ScraperEngine`; `jobs_cancel` reaches them                                                                                                                                                                                                                                                           |
| Streaming `agent:step` events    | ✅     | Per-turn narration (plan + tool calls) and terminal proposal, streamed to the Job Detail pane (`PrepApplicationPanel`)                                                                                                                                                                                                                                           |
| Prompt normalization (Rust)      | ✅     | System + user prompts in Rust; per-flow in `flows.rs`. See `draft_cover_letter` / `suggest_interview_questions` compact Rust implementations vs TS `@ajh/prompts` builders (drift risk)                                                                                                                                                                          |
| Interview practice mode          | ✅     | Mock questions + star answer feedback via `suggest_interview_questions` tool; interactive drill UI for candidate preparation (PR #623, v0.126.0)                                                                                                                                                                                                                 |

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
| Documents route           | ✅     | Three-tab view — Résumés / Cover Letters / Activity (lenses over `ai_generations`); canonical route is `/documents` (`createFileRoute('/documents')`); no `/resumes` route exists               |
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

## Applications & Tracker

Persistent job-application records with rich metadata and automation.

| Feature                             | Status | Notes                                                                                                                                     |
| ----------------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Application record model            | ✅     | Per-job Application aggregate; linked to ai_generations, interaction_history, interview_questions, answers                                |
| Status pipeline                     | ✅     | saved → applied → (interviewing/rejected/offer/accepted); auto-derived `applied` from `ai_generations.job_url`                            |
| Saved answers per job               | ✅     | ApplicationAnswer array persisted; extension suggests matches for new applications (extension_bridge answers_suggest)                     |
| Interview Q&A storage               | ✅     | InterviewQuestion array on Application aggregate; suggested via `suggest_interview_questions` agent tool                                  |
| Candidate questions for interviewer | ✅     | Suggest interview questions to ask the interviewer; stored on Application for reuse (PR #383, v0.104.0 + practice mode PR #623, v0.126.0) |

---

## Email-Confirmation Watching (Auto-Track Layer C)

Auto-detect applications via email confirmation watches (IMAP polling foundation).

| Feature                    | Status | Notes                                                                                                                |
| -------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------- |
| Email provider connection  | ✅     | Gmail via IMAP app password (OS keychain, no OAuth — ADR-0013 records why); integrated into Settings (PR #689)       |
| IMAP store + polling       | 🚧     | EmailWatchStore shipped (#689); poller/parser/matcher + startup watcher built, in review (Layer C PR B); ADR-0013    |
| Confirmation email parsing | 🚧     | Template-driven regex matchers for job board confirmation emails (in-progress, deferred: multi-board matcher tuning) |

---

## Browser Extension (Chrome + Firefox)

MV3 extension (`apps/extension`) published on Chrome Web Store + Firefox AMO; bridges desktop via loopback native messaging with HMAC authentication (ADR-0010, PR #627).

| Feature                   | Status | Notes                                                                                                                                   |
| ------------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------- |
| Job import from boards    | ✅     | Canonical URL resolution for LinkedIn/Indeed (PR #390); deferred: Glassdoor/Xing/StepStone                                              |
| Opt-in contact autofill   | ✅     | Click-to-fill contact profile on any form; two-gate consent, fetch-fresh no-persist, never-submit; generic matcher (PR #625, ADR-0009)  |
| Answers capture + replay  | ✅     | Save application-form question/answer pairs; suggest matched answers from prior applications (PR #637, deferred: HMAC bridge signature) |
| AI answer draft + rewrite | ✅     | Generate or rewrite application answers from the extension with streaming; opt-in gates per question (PR #649, #675)                    |
| Check-fit scoring         | ✅     | Resume matching against job ad; displayed inline on job posting                                                                         |
| Mark as applied           | ✅     | Record application from the job board                                                                                                   |
| Auto-track on form submit | ✅     | Opt-in listener detects form submissions, auto-records application (Layer A, PR #687, v0.126.0)                                         |
| HMAC bridge protocol v2   | ✅     | Mutual HMAC-SHA256 challenge-response; token never on wire; closes port-squat deferral (ADR-0010, PR #627)                              |

---

## Landing & Public Site

Next.js static-export workspace package serving brand, download links, and documentation.

| Feature                    | Status | Notes                                                                                                                                          |
| -------------------------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| Next.js static export      | ✅     | `apps/landing/` is a Next.js 16.2.10 package (`output: 'export'`); flat files (no server runtime); PR1 delivered                               |
| TypeScript version pairing | ✅     | TypeScript 6.0.3 pinned to root + apps/landing (Next 16's verifyTypeScriptSetup incompatible with 7.x); desktop/extension use 7.x per-importer |
| Authored pages as routes   | ✅     | 5 pages: home, creature, how-it-works, privacy, download (all `src/app/`); faithful port of legacy static site                                 |
| Passthrough artifacts      | ✅     | Benchmarks, storybook copied verbatim from `public/` (CI-owned, not built by Next); architecture-map port deferred                             |
| Parity gate                | ✅     | `pnpm check:parity` ensures byte-shape parity with legacy static layout (permanent, non-optional pre-push/CI gate)                             |
| GitHub Pages deployment    | ✅     | `pages.yml` publishes Next.js export output (`out/`) directly to Pages                                                                         |
| Release seam               | ✅     | `src/data/version.json` baked at build time; `/download` and homepage read for client-side freshness checks                                    |
| Brand tokens               | ✅     | Paper/ink/red palette, self-hosted fonts (`public/fonts/`, PR2), film-grain overlay; shared with extension store assets; marketing tier        |
| Docs tier (PR2)            | ✅     | `/mission-control` full-repo dashboard shipped (clean URL rename, no redirect stub); PAT sign-in + safe-tier writes                            |
| DocShell + tokens (PR2)    | ✅     | Unified docs-tier visual language (dark hand-drawn look); typed-data route for agent-system (`src/data/agent-fleet.ts`)                        |
| /how-it-works reskin (PR2) | ✅     | Ported to DocShell; visual consistency with mission-control                                                                                    |
| OG template (PR3)          | ✅     | `social-card.html` relocated to `scripts/assets/` beside its generator; no longer app content                                                  |
| Architecture-map port      | ⬜     | Deferred to follow-up PR (remains passthrough artifact); design intent unchanged                                                               |

**Note:** TERMINAL VELOCITY scroll-film (ADR 0016, merged M1–M3) abandoned 2026-07-20 mid-M4.
All film concepts (playhead, scroll-film, scenes, quality governor, VAT shaders) and
Experience-gate machinery (ADR 0014) retired. Static site (now Next.js) remains the sole public landing (ADR-0018).

---

## Planned / Backlog

| Feature                                 | Priority | Notes                                                                                                                                                             |
| --------------------------------------- | -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Cluster split undo (merge-back)         | Low      | Pair-tombstone delete + recompute; deferred from ADR-029                                                                                                          |
| CJK font support in PDF/preview         | Medium   | Bundle Noto Sans CJK into Typst engine (`export/typst_engine/`) so zh/ja/ko render in PDF and live preview (generation + DOCX already work; currently shows tofu) |
| URL-to-job-ad extraction in AI Generate | Medium   | `scrape.url` IPC contract exists; UI input not yet wired                                                                                                          |
| LinkedIn official API integration       | Medium   | Currently Playwright-only                                                                                                                                         |
| Advanced skill taxonomy                 | Medium   | Structured ontology for matching                                                                                                                                  |
| Derive AUTH_BOARDS from catalog         | Low      | AUTH_BOARDS (renderer constants/auth/auth.ts) is hardcoded LinkedIn-only; derive from SCRAPERS board catalog (REQ-16630)                                          |
| Wire `scrape:item` listener             | Low      | `scrape:item` emitted from autopilot_helpers but no renderer subscriber exists; `scrape:progress` wired in #564, complete the pair                                |
| Persist Jobs ScrapeForm state           | Low      | ScrapeForm state (JobsPage-local useState) resets on navigation; unlike filter/sort persist, should survive route changes                                         |
| Drop dead `totalApplied` counter        | Low      | Legacy persisted counter in packages/shared/src/types and Rust Autopilot struct (serde ignores it; remove both)                                                   |
| Autopilot battery-awareness             | Low      | Pause heavy scraping on battery power; add battery/AC check + allow-on-battery preference (default: pause)                                                        |
| i18n OllamaResourcesPanel               | Low      | RAM/VRAM labels and lag warnings hardcoded English in ai-settings/AISettingsTab/OllamaResourcesPanel.tsx                                                          |
| Ai_provider module relocation           | Low      | Relocate ai_provider (~1,450 LOC) from commands/ai_provider/ to top-level src/ai_provider/ L1 module with thin wrappers in commands/ai.rs (architecture)          |
| Burn down Tauri-coupling allowlist      | Low      | 8-entry R2 allowlist in tests/architecture.rs (tauri emit/Manager in non-shell modules); inject emitter/resource port per ADR-0025                                |
| E2E data backup round-trip test         | Low      | REQ-13006: add verify/E2E test for export→re-import full bundle round-trip (only per-store unit tests exist; needs versioned bundle test)                         |
| Cloud sync                              | Low      | Deferred — needs a remote backend; the backup bundle + `DataStore` trait are the substrate                                                                        |
| Team/shared job tracking                | Low      | Would require cloud sync                                                                                                                                          |

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
