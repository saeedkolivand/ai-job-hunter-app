// Single source of truth for the /tech-radar page — a curated, human-judged
// list (not derived from package.json, deliberately: the whole point of a
// radar is the judgment call, same as a changelog or an ADR). Two things keep
// a curated list honest instead of letting it rot silently:
//
//   1. scripts/check-tech-radar.mjs (CI, wired into ci-pipeline.yml + the
//      pre-push hook) fails the build when a `subjectKind: 'dependency'`
//      entry names a package that no longer exists in any package.json /
//      Cargo.toml on disk — update the entry or remove it.
//   2. `adrSlug` is checked against docs/adr/ the same way — a dead link
//      fails loudly instead of quietly pointing nowhere.
//
// Every entry object below is intentionally FLAT (no nested objects/arrays)
// so the checker can bound one entry with a simple non-nested-brace regex —
// same convention as the node blocks in ./architecture-map.ts (see that
// file's header comment and scripts/check-landing-drift.mjs's
// CONTRACT_BLOCK_RE for why that convention exists).
//
// Rings follow the standard tech-radar convention (innermost = most settled):
// Adopt → Trial → Assess → Hold. Quadrants are named for THIS project's real
// shape rather than Thoughtworks' generic set — see QUADRANTS below.

export type RadarRing = 'adopt' | 'trial' | 'assess' | 'hold';

export type RadarQuadrant =
  'renderer-ui' | 'backend-data' | 'documents-export' | 'build-ship-trust';

// What scripts/check-tech-radar.mjs should do with an entry:
//  - 'dependency': `dependencyName` (or `name` if omitted) MUST be a real
//    dependency key in a package.json / Cargo.toml on disk today.
//  - 'technique': an in-house pattern or practice — no package name exists.
//  - 'service': a hosted or locally-run service reached over HTTP, not
//    installed via a package manager (Ollama, CodeRabbit, Nominatim, …).
//  - 'not-adopted': names a REAL package/product that was deliberately never
//    added (or was removed on purpose) — exempt from the dependency check by
//    design, not because nobody wrote the check for it.
export type RadarSubjectKind = 'dependency' | 'technique' | 'service' | 'not-adopted';

export interface TechRadarEntry {
  id: string;
  name: string;
  ring: RadarRing;
  quadrant: RadarQuadrant;
  subjectKind: RadarSubjectKind;
  dependencyName?: string;
  summary: string;
  rationale: string;
  adrSlug?: string;
  lastReviewed: string;
}

export const QUADRANTS: readonly { id: RadarQuadrant; label: string }[] = [
  { id: 'renderer-ui', label: 'Renderer & UI' },
  { id: 'backend-data', label: 'Rust Core & Data' },
  { id: 'documents-export', label: 'Documents & Export' },
  { id: 'build-ship-trust', label: 'Build, Ship & Trust' },
];

export const RINGS: readonly { id: RadarRing; label: string; blurb: string }[] = [
  {
    id: 'adopt',
    label: 'Adopt',
    blurb: 'In production today — the default choice for new work in this area.',
  },
  {
    id: 'trial',
    label: 'Trial',
    blurb: 'Shipping, with a specific caveat or a partial rollout worth knowing about.',
  },
  {
    id: 'assess',
    label: 'Assess',
    blurb: 'Not in the codebase yet — worth understanding and watching before committing.',
  },
  {
    id: 'hold',
    label: 'Hold',
    blurb: 'Deliberately not used here. Proceed with caution; the reasoning is in the entry.',
  },
];

export const RADAR: readonly TechRadarEntry[] = [
  // ── Renderer & UI ───────────────────────────────────────────────────────
  {
    id: 'tauri',
    name: 'Tauri 2',
    ring: 'adopt',
    quadrant: 'renderer-ui',
    subjectKind: 'dependency',
    dependencyName: 'tauri',
    summary: 'OS-native WebView desktop shell — not Electron.',
    rationale:
      "Rust backend plus the OS's own WebView (WebView2 on Windows, WebKit on macOS/Linux) instead of bundling Chromium: installers land in the tens of MB rather than 150+ MB, and the renderer only reaches Rust through commands explicitly allowed by Tauri's capability manifest. The trade-off — WebKit on macOS doesn't always render identically to WebView2 — is handled with a cross-platform CSS pass before each release.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'react',
    name: 'React 19',
    ring: 'adopt',
    quadrant: 'renderer-ui',
    subjectKind: 'dependency',
    dependencyName: 'react',
    summary: 'Concurrent rendering, Actions, first-class TanStack support.',
    rationale:
      'The renderer is React 19.2 throughout apps/desktop and apps/landing — Actions/useActionState for async transitions, ref as a plain prop, and the concurrent-safe integration TanStack Query and Zustand both depend on.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'tanstack-query',
    name: 'TanStack Query 5',
    ring: 'adopt',
    quadrant: 'renderer-ui',
    subjectKind: 'dependency',
    dependencyName: '@tanstack/react-query',
    summary: 'The only sanctioned way for a component to reach IPC.',
    rationale:
      'Every server-state read/write goes through a service hook in renderer/services/ — no useState + useEffect fetching, no direct window.api access from features/routes/components. Query keys are centralized so cache invalidation after a mutation is deliberate, not guessed at.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'tanstack-router',
    name: 'TanStack Router',
    ring: 'adopt',
    quadrant: 'renderer-ui',
    subjectKind: 'dependency',
    dependencyName: '@tanstack/react-router',
    summary: 'File-based routing with typed route + search params.',
    rationale:
      'Chosen for compile-time-checked routes and search-param types over a stringly-typed router API — the desktop app has 11+ feature routes, and a renamed route param fails at build time instead of at runtime.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'zustand',
    name: 'Zustand 5',
    ring: 'adopt',
    quadrant: 'renderer-ui',
    subjectKind: 'dependency',
    dependencyName: 'zustand',
    summary: 'Minimal client-only state — persisted prefs, transient session.',
    rationale:
      "Used over Redux because the client-state slices (persisted preferences, the transient generation session) are simple enough that Redux's action/reducer boilerplate bought nothing. Zustand is a plain hook with no provider tree, and it's React 19 concurrent-safe.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'tailwind',
    name: 'Tailwind CSS 4',
    ring: 'adopt',
    quadrant: 'renderer-ui',
    subjectKind: 'dependency',
    dependencyName: 'tailwindcss',
    summary: 'CSS-first @theme config — the design-token backbone.',
    rationale:
      "packages/ui defines every design token (--color-brand, --color-surface-elevated, …) as a CSS custom property consumed via Tailwind 4's @theme. ESLint then bans a raw hex color in any className, so a color can't drift from the token file without the linter catching it.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'motion',
    name: 'Motion (motion/react)',
    ring: 'adopt',
    quadrant: 'renderer-ui',
    subjectKind: 'dependency',
    dependencyName: 'motion',
    summary: 'Animation library, wrapped behind named transition tokens.',
    rationale:
      'packages/ui/src/lib/motion.ts exposes named presets (transition.fast/.spring/.modal/…) over the raw library; ESLint blocks an inline { duration, ease } object anywhere in feature code, so every animation in the app traces back to one small token file.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'custom-state-machine',
    name: 'Hand-rolled state machine (lib/machine.ts)',
    ring: 'adopt',
    quadrant: 'renderer-ui',
    subjectKind: 'technique',
    summary: '~80-line machine + a useMachine hook for any 3+-state flow.',
    rationale:
      'Any flow with 3+ states (onboarding, streaming generation) gets a state machine. Named states replace boolean tangles (isLoading && isDone) and make an impossible state impossible to represent, without pulling in a general-purpose library — see XState, held, for why not that one.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'xstate',
    name: 'XState',
    ring: 'hold',
    quadrant: 'renderer-ui',
    subjectKind: 'not-adopted',
    summary: 'Considered for the same job as our micro state machine — held.',
    rationale:
      "The flows in this app top out at a handful of linear states, so XState's parallel states, history, and guards buy nothing today, at the cost of bundle weight and its own config DSL. Kept as an explicit option if a flow ever genuinely needs what it offers — this entry exists so that reasoning stays visible instead of getting re-litigated.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'wcag22aa',
    name: 'WCAG 2.2 AA',
    ring: 'adopt',
    quadrant: 'renderer-ui',
    subjectKind: 'technique',
    summary: 'The non-negotiable accessibility floor for every route.',
    rationale:
      "Enforced via eslint-plugin-jsx-a11y, @axe-core/playwright, and apps/landing's own check:a11y script; /accessibility publishes the conformance statement. Colour is never the sole signal for state — including on this very page's ring encoding — and every interactive element ships a visible :focus-visible ring.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'webgl-landing',
    name: 'WebGL / shader-driven landing experiences',
    ring: 'hold',
    quadrant: 'renderer-ui',
    subjectKind: 'technique',
    summary: 'Two hand-built WebGL landing pieces were built, then shelved.',
    rationale:
      'TERMINAL VELOCITY (a scroll-driven CG retelling of the job hunt) and RIPBOOK (a notebook-styled concept) were each built across real milestones and then deliberately abandoned before their visual approval gates passed — the landing site returned to a plain static/Next export. The dormant webgl-author/shader-engineer/webgl-reviewer agent trio stays dormant; a returning WebGL surface would route back to them, not the general frontend author.',
    adrSlug: '0017-landing-consolidation-static-site',
    lastReviewed: '2026-08-05',
  },

  // ── Rust Core & Data ────────────────────────────────────────────────────
  {
    id: 'rusqlite',
    name: 'SQLite via rusqlite (bundled)',
    ring: 'adopt',
    quadrant: 'backend-data',
    subjectKind: 'dependency',
    dependencyName: 'rusqlite',
    summary: 'The whole local-first store — no external DB process.',
    rationale:
      "rusqlite's bundled feature ships SQLite inside the binary, so there's no version mismatch or separate database process to install. Seven independent databases (documents, conversations, ai_generations, job_preferences, contact_profile, jobs, pipeline_cache) keep faults isolated — a corrupt conversations.db doesn't touch documents.db.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'tokio',
    name: 'Tokio',
    ring: 'adopt',
    quadrant: 'backend-data',
    subjectKind: 'dependency',
    dependencyName: 'tokio',
    summary: 'Async runtime for board scraping and background jobs.',
    rationale:
      'Board scrapers run concurrently via tokio::spawn, each holding a CancellationToken so a scrape can be stopped mid-run; long operations are tracked by a SQLite-backed job tracker with retry rather than blocking command handling.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'keyring-core',
    name: 'keyring-core (OS-native credential storage)',
    ring: 'adopt',
    quadrant: 'backend-data',
    subjectKind: 'dependency',
    dependencyName: 'keyring-core',
    summary: 'API keys and board passwords never touch the renderer.',
    rationale:
      "Credentials live in the OS keychain (Credential Manager/DPAPI on Windows, Keychain on macOS, libsecret on Linux) via keyring-core's platform adapters. The renderer calls credential commands over IPC and never handles a raw secret — a renderer XSS can't reach them because they live outside the web context entirely.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'chromiumoxide',
    name: 'chromiumoxide (headless Chromium automation)',
    ring: 'adopt',
    quadrant: 'backend-data',
    subjectKind: 'dependency',
    dependencyName: 'chromiumoxide',
    summary: 'Drives a real browser for boards that block plain HTTP.',
    rationale:
      'LinkedIn is scraped over plain HTTP, and the walled boards (Indeed, Glassdoor, StepStone, Xing, Workday) mostly go through the Adzuna/JSearch aggregator — chromiumoxide backs the specific boards and login flows that genuinely need a real, scriptable browser rather than a bare HTTP client.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'reqwest',
    name: 'reqwest',
    ring: 'adopt',
    quadrant: 'backend-data',
    subjectKind: 'dependency',
    dependencyName: 'reqwest',
    summary: 'The one HTTP client every scraper and AI call goes through.',
    rationale:
      'Centralized in net/http.rs so every outbound call — scraping, AI providers, geocoding — shares one client, one timeout policy, and one place to audit against the network-egress boundary.',
    adrSlug: '0005-network-egress-privacy-boundary',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'zod',
    name: 'Zod',
    ring: 'adopt',
    quadrant: 'backend-data',
    subjectKind: 'dependency',
    dependencyName: 'zod',
    summary: 'Schema-first validation at every IPC and form boundary.',
    rationale:
      "IPC payloads and form data are validated with Zod at the boundary (IPC receive, form submit); inside the app the inferred TypeScript types are trusted, so component logic isn't full of defensive if (!data?.id) checks.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'ollama',
    name: 'Ollama (local + cloud)',
    ring: 'adopt',
    quadrant: 'backend-data',
    subjectKind: 'service',
    summary: 'The offline-first AI provider — a local model, not just an API key.',
    rationale:
      "The one AI provider that needs no API key and no network call at all: a locally-run model the app talks to over loopback HTTP, with an optional Ollama Cloud mode alongside it. It's the concrete reason 'no API key yet' doesn't mean 'no AI features yet' for a local-first app.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'live-model-listing',
    name: 'Live model listing, no hardcoded defaults',
    ring: 'adopt',
    quadrant: 'backend-data',
    subjectKind: 'technique',
    summary: 'Every provider model list is fetched live — never a curated array.',
    rationale:
      "Four stale-model defects shipped in one session from hand-curated model arrays (a retired embedding model left as the default, a shut-down Gemini preview, its equally-dead list neighbours). ADR-0022 deleted every hardcoded array; a provider's own /models endpoint is now the only source, and onboarding pre-selects nothing.",
    adrSlug: '0022-live-model-listing-no-hardcoded-defaults',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'dedicated-vector-db',
    name: 'A dedicated vector database',
    ring: 'hold',
    quadrant: 'backend-data',
    subjectKind: 'not-adopted',
    summary: 'Considered for posting-embedding search — held.',
    rationale:
      'Posting embeddings are stored in SQLite alongside the documents and searched with an in-memory cosine pass, not in Pinecone/pgvector/a standalone vector engine — the corpus a single local user accumulates is small enough that the extra dependency and the extra moving part it would add buy nothing measurable today.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'nominatim',
    name: 'Nominatim',
    ring: 'hold',
    quadrant: 'backend-data',
    subjectKind: 'service',
    summary: 'Retired as the geocoding fallback — its usage policy forbids autocomplete.',
    rationale:
      'Location autocomplete answers offline from a bundled GeoNames index for virtually every query; only a genuine miss falls through to Photon (OpenStreetMap-backed) as the network fallback. Nominatim filled that fallback role first and was retired because its usage policy explicitly forbids autocomplete-style querying.',
    adrSlug: '0005-network-egress-privacy-boundary',
    lastReviewed: '2026-08-05',
  },

  // ── Documents & Export ──────────────────────────────────────────────────
  {
    id: 'typst',
    name: 'Typst (typst / typst-pdf / typst-layout / typst-svg)',
    ring: 'adopt',
    quadrant: 'documents-export',
    subjectKind: 'dependency',
    dependencyName: 'typst',
    summary: 'One pure-Rust engine renders every résumé and cover-letter PDF.',
    rationale:
      'A single Typst engine backs both documents so résumé and cover-letter output share one layout system instead of two. The whole family is exact-pinned to =0.15.1 in lockstep — a solo version bump anywhere in it is a red flag, not routine maintenance.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'docx-rs',
    name: 'docx-rs',
    ring: 'adopt',
    quadrant: 'documents-export',
    subjectKind: 'dependency',
    dependencyName: 'docx-rs',
    summary: "Native DOCX generation for the résumé's second export format.",
    rationale:
      "Renders the canonical document model straight to a real two-column DOCX table with native ATS-mode support, guarded by golden invariants so parity between the DOCX and PDF paths doesn't silently drift.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'lopdf',
    name: 'lopdf',
    ring: 'adopt',
    quadrant: 'documents-export',
    subjectKind: 'dependency',
    dependencyName: 'lopdf',
    summary: 'Low-level PDF manipulation — including inline annotation dicts.',
    rationale:
      "Used where PDF structure needs direct manipulation rather than pure rendering; inline (non-referenced) /Annots dictionaries needed custom parsing since lopdf's own annotation handling assumes the referenced form.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'pdf-extract',
    name: 'pdf-extract',
    ring: 'adopt',
    quadrant: 'documents-export',
    subjectKind: 'dependency',
    dependencyName: 'pdf-extract',
    summary: 'Text extraction for imported PDF résumés.',
    rationale:
      'Backs step one of the document-import pipeline (format detection → text extraction → SQLite storage → chunking → embedding) for PDF specifically; DOCX and images go through their own dedicated parser/OCR paths.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'image-crate',
    name: 'image (Rust crate)',
    ring: 'adopt',
    quadrant: 'documents-export',
    subjectKind: 'dependency',
    dependencyName: 'image',
    summary: 'Raster handling for OCR input and export assets.',
    rationale:
      "Exact-pinned alongside the Typst family (=0.25.10) since Typst's own SVG/PDF export path depends on it — kept in lockstep rather than left to float independently.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'dual-engine-parity',
    name: 'Dual-engine golden-parity migration',
    ring: 'adopt',
    quadrant: 'documents-export',
    subjectKind: 'technique',
    summary: 'The legacy renderer stays compiled as the parity reference.',
    rationale:
      "layout_pdf and model_docx are on by default now that the canonical layout engine has snapshot parity with the legacy line-based renderer, but the legacy path stays compiled — it's still the parity reference, and it still renders cover letters — so --no-default-features can fall back to it if the canonical path ever regresses.",
    lastReviewed: '2026-08-05',
  },

  // ── Build, Ship & Trust ─────────────────────────────────────────────────
  {
    id: 'typescript',
    name: 'TypeScript',
    ring: 'trial',
    quadrant: 'build-ship-trust',
    subjectKind: 'dependency',
    dependencyName: 'typescript',
    summary: '7.x (native compiler) everywhere except apps/landing, pinned to 6.x.',
    rationale:
      "apps/desktop and every packages/* workspace run TypeScript 7's native compiler; apps/landing stays pinned to 6.0.3 because Next 16's build-time verifyTypeScriptSetup doesn't recognise TS 7's native-compiler package layout and crashes the build worker. Trial, not Adopt, until Next supports it and the pin can drop repo-wide.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'vite',
    name: 'Vite 8',
    ring: 'adopt',
    quadrant: 'build-ship-trust',
    subjectKind: 'dependency',
    dependencyName: 'vite',
    summary: 'Dev server + build tool for the desktop renderer, landing, and the extension.',
    rationale:
      'Every buildable frontend workspace (apps/desktop, apps/landing, apps/extension, packages/ui) builds on Vite; Vitest is Vite-native, so the same config/plugin surface backs both dev and test.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'vitest',
    name: 'Vitest 4',
    ring: 'adopt',
    quadrant: 'build-ship-trust',
    subjectKind: 'dependency',
    dependencyName: 'vitest',
    summary: 'The one test runner, in every workspace.',
    rationale:
      'A root vitest workspace config aggregates every package + app project — plus a dedicated node-env project for build/release scripts — into one coverage report, so `pnpm test` is a single command regardless of which package changed.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'playwright',
    name: 'Playwright',
    ring: 'adopt',
    quadrant: 'build-ship-trust',
    subjectKind: 'dependency',
    dependencyName: 'playwright',
    summary: "Axe-driven accessibility checks, and the desktop app's E2E suite.",
    rationale:
      "Backs apps/landing's check:a11y (@axe-core/playwright) and apps/desktop's Playwright E2E suite (test:e2e) — the same tool covers both a static export and a live Tauri window.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'turborepo',
    name: 'Turborepo',
    ring: 'adopt',
    quadrant: 'build-ship-trust',
    subjectKind: 'dependency',
    dependencyName: 'turbo',
    summary: 'Incremental builds across an 8-workspace monorepo.',
    rationale:
      "Tracks file hashes per package, so an unchanged packages/shared build is skipped entirely when only apps/desktop changed — the dependency graph is why CI build time doesn't scale linearly with workspace count.",
    lastReviewed: '2026-08-05',
  },
  {
    id: 'semantic-release',
    name: 'semantic-release',
    ring: 'adopt',
    quadrant: 'build-ship-trust',
    subjectKind: 'dependency',
    dependencyName: 'semantic-release',
    summary: 'Commit-driven, manually-triggered versioning — never automatic.',
    rationale:
      'Driven by Conventional Commits (feat → minor, fix/perf → patch, BREAKING CHANGE → minor pre-1.0), but nothing runs on push/merge to main — a release is a deliberate Actions dispatch, never an automatic side effect of merging.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'husky-lint-staged',
    name: 'Husky + lint-staged pre-commit gate',
    ring: 'adopt',
    quadrant: 'build-ship-trust',
    subjectKind: 'dependency',
    dependencyName: 'husky',
    summary: 'Every commit is linted/formatted before it lands, not after.',
    rationale:
      'Pre-commit runs eslint --fix on staged TypeScript and Prettier on the rest; commitlint checks the message. Pre-push runs the full gate (typecheck, lint, cargo check/test/clippy, formatting) so main never carries a known lint or type error.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'eslint-astgrep',
    name: 'ESLint + ast-grep architecture guardrails',
    ring: 'adopt',
    quadrant: 'build-ship-trust',
    subjectKind: 'dependency',
    dependencyName: 'eslint',
    summary: 'The rules in AGENTS.md are enforced, not just written down.',
    rationale:
      'Package-boundary imports, the ports-and-adapters window.api ban, hardcoded hex colors, inline transition objects, raw <button>/<select>/<textarea> — every one of those rules is an ESLint error or an ast-grep scan rule, not a convention someone has to remember and a reviewer has to catch by eye.',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'sentry',
    name: 'Sentry crash reporting',
    ring: 'adopt',
    quadrant: 'build-ship-trust',
    subjectKind: 'dependency',
    dependencyName: 'sentry',
    summary: 'Desktop-only, default ON, consent-gated, whole-event redacted.',
    rationale:
      'Adding remote crash reporting reversed a published no-telemetry promise, so the decision to do it is recorded explicitly: default on, but nothing transmits until the first-run wizard has actually shown the consent screen, every outgoing event is redacted, the DSN is baked only into signed release builds, and both the browser extension and the landing site are excluded.',
    adrSlug: '0020-crash-reporting',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'coderabbit',
    name: 'CodeRabbit',
    ring: 'adopt',
    quadrant: 'build-ship-trust',
    subjectKind: 'service',
    summary: 'The always-on AI PR reviewer — advisory only, never blocking.',
    rationale:
      "Free and unlimited on public repos, and it overlapped three separate advisory lanes (a reviewdog ESLint/Clippy pass, a Dangerfile, an actionlint lane) closely enough that all three were retired in its favour. It's configured to never approve or block on its own — the required check stays CI, plus an on-demand deep-dive review for anything that needs one.",
    adrSlug: '0002-coderabbit-ai-review',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'nextjs-static-export',
    name: 'Next.js (static export)',
    ring: 'adopt',
    quadrant: 'build-ship-trust',
    subjectKind: 'dependency',
    dependencyName: 'next',
    summary: 'apps/landing is plain HTML/JS at runtime — no server, ever.',
    rationale:
      "output: 'export' — no middleware, no Server Actions, no ISR, no dynamic route handlers, no headers()/cookies(). A permanent check:parity gate diffs the built out/ against the legacy static site so a route can never silently change shape.",
    adrSlug: '0018-landing-nextjs-static-export',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'codecov-sonarcloud',
    name: 'Codecov + SonarCloud',
    ring: 'hold',
    quadrant: 'build-ship-trust',
    subjectKind: 'not-adopted',
    summary: 'Dropped in favour of the zero-external-SaaS CI default.',
    rationale:
      "The CI program's stated default is Actions-native, zero external SaaS advisory tooling; both were dropped for exactly that reason before CodeRabbit was even evaluated. CodeRabbit's own free-tier scan (ESLint, Clippy, Semgrep, secret-scan) made a paid hosted coverage/quality dashboard even less necessary once it was adopted.",
    adrSlug: '0002-coderabbit-ai-review',
    lastReviewed: '2026-08-05',
  },
  {
    id: 'react-compiler',
    name: 'React Compiler',
    ring: 'assess',
    quadrant: 'build-ship-trust',
    subjectKind: 'technique',
    summary: 'GA since React 19, not yet wired into any build here.',
    rationale:
      "No babel-plugin-react-compiler or eslint-plugin-react-compiler dependency exists anywhere in the monorepo today — manual memoization discipline is still how the renderer avoids re-render cost. Worth a real trial once it's had more mileage against a render-heavy, streaming-text-updating UI like this one; not adopted yet because nobody has actually run it here.",
    lastReviewed: '2026-08-05',
  },
];
