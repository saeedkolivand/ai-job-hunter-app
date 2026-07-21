// Single source of truth for the /agent-system page (ported from the former
// hand-authored public/agent-system.html). Also the drift-guard EXPLAINER target
// for scripts/check-agent-system.mjs — every `.claude/agents/*.md` name must
// appear here (check 6), and the roster tuples below feed its reverse-check
// (check 9). Keep the `[name, role, …]` tuple shape so that guard keeps working.

export type AgentRole = 'author' | 'critic' | 'cross';

// [name, role, whatItDoes, pairing, paths, delegateExample]
export type AgentTuple = readonly [string, AgentRole, string, string, string, string];

export const AUTHORS: readonly AgentTuple[] = [
  [
    'rust-backend-author',
    'author',
    'Implements the Rust/Tauri backend — domain modeling, errors, module boundaries, SQLite/migrations.',
    '↔ critic rust-backend-architect (+ tauri-security-reviewer on risk)',
    'apps/desktop/src-tauri/src/** · packages/shared/**',
    'Use rust-backend-author to add a SQLite-backed store for saved jobs.',
  ],
  [
    'frontend-author',
    'author',
    'Implements the React renderer — components, routes, UI state; design-system + i18n + a11y compliant.',
    '↔ critics frontend-reviewer + ui-ux-expert',
    'apps/desktop/src/renderer/** · packages/ui/**',
    'Use frontend-author to add a saved-jobs panel with a folder picker.',
  ],
  [
    'job-match-author',
    'author',
    'Implements ATS scoring, job analysis, keyword/skill extraction, matching, and cover-letter relevance.',
    '↔ critic job-match-expert',
    'commands/match_resume.rs · cover_letter/** · recommend/** · validate/**',
    'Use job-match-author to add a keyword-coverage tag to each saved job.',
  ],
  [
    'ai-provider-author',
    'author',
    'Implements AI provider integrations, model routing, embeddings, prompts, and streaming.',
    '↔ critic ai-provider-expert',
    'commands/ai_provider/** · commands/ai.rs · documents/** · packages/prompts/**',
    'Use ai-provider-author to wire a new model into the provider registry.',
  ],
  [
    'scraping-applier-author',
    'author',
    'Implements job scraping + browser automation, selector resilience, and the SCRAPERS registry.',
    '↔ critic scraping-applier-expert',
    'apps/desktop/src-tauri/src/scraping/** · browser/**',
    'Use scraping-applier-author to fix the LinkedIn list-view selectors.',
  ],
  [
    'pdf-docx-generator',
    'author',
    'Implements export rendering — PDF/DOCX, layout, fonts, pagination, golden snapshots, DocumentModel/theme/locale.',
    '↔ critic resume-export-expert',
    'export/** · model/** · theme/** · locale/** · layout/** · measure/**',
    'Use pdf-docx-generator to fix the résumé overflowing onto a second page.',
  ],
  [
    'test-author',
    'author',
    'Writes automated tests — unit / integration / e2e / golden — across every domain.',
    '↔ critic testing-reviewer',
    '**/*.test.* · **/*.spec.* · src-tauri/tests/**',
    'Use test-author to add golden tests for the new export path.',
  ],
  [
    'code-quality-author',
    'author',
    'Refactors to meet clean-code / DRY / KISS — smallest behavior-preserving diff.',
    '↔ critic code-quality-reviewer',
    'any package/path on request',
    'Use code-quality-author to deduplicate the repeated scoring helpers.',
  ],
  [
    'extension-author',
    'author',
    'Implements the browser extension (MV3, Chrome + Firefox) and the desktop↔extension bridge (native-host + loopback WS, per-frame token auth) plus the shared wire protocol.',
    '↔ critic extension-reviewer (+ tauri-security-reviewer on auth/permission/data risk)',
    'apps/extension/** · apps/desktop/src-tauri/src/extension_bridge/** · packages/shared/src/ipc/extension-protocol*',
    'Use extension-author to add a new extension message type.',
  ],
  [
    'webgl-author',
    'author',
    'Implements the apps/landing WebGL experience — scenes, engine, scroll rig, a11y overlay, semantic layer, content, and fallback; everything scroll-driven as a pure function of t.',
    'reviewed by webgl-reviewer (+ gate-auditor on rendered output)',
    'apps/landing/src/** — scenes, engine, a11y, semantic (not GLSL/post)',
    'Use webgl-author to add a new landing scene and wire it into the scroll rig.',
  ],
  [
    'shader-engineer',
    'author',
    'Owns all GLSL in apps/landing — material shaders, the post-processing chain, procedural textures, and vertex shaders; writes shaders to spec, never touches scene layout or engine wiring.',
    'reviewed by webgl-reviewer',
    'apps/landing GLSL — materials, post chain, procedural textures',
    'Use shader-engineer to write a new material shader for a landing scene.',
  ],
];

export const CRITICS: readonly AgentTuple[] = [
  [
    'rust-backend-architect',
    'critic',
    'Reviews the Rust/Tauri backend — domain modeling, error handling, L0–L3 boundaries, data/SQLite/GDPR.',
    'audits rust-backend-author',
    'apps/desktop/src-tauri/src/** (unowned) · packages/shared/**',
    '/review-rust on the new store command.',
  ],
  [
    'frontend-reviewer',
    'critic',
    'Reviews the React renderer only — ports-&-adapters, design system, React Query, i18n, a11y.',
    'audits frontend-author',
    'apps/desktop/src/renderer/** · packages/ui/**',
    '/review-frontend on the saved-jobs panel.',
  ],
  [
    'ui-ux-expert',
    'critic',
    'The visual + UX + deep-a11y taste lens — hierarchy, spacing, motion, microcopy. Read-only.',
    'secondary on frontend-author changes',
    'apps/desktop/src/renderer/** · packages/ui/** · landing/**',
    'Use ui-ux-expert to critique the folder-picker UX.',
  ],
  [
    'job-match-expert',
    'critic',
    'Reviews ATS scoring / job analysis / matching / cover-letter relevance.',
    'audits job-match-author',
    'match_resume.rs · cover_letter.rs · validate/ · documents/embed',
    '/review-ats on the keyword-coverage tag.',
  ],
  [
    'ai-provider-expert',
    'critic',
    'Reviews provider integrations — enforces add-a-provider = config + adapter only, never business-logic coupling.',
    'audits ai-provider-author',
    'ai_provider/** · commands/ai.rs · documents/embed · packages/prompts',
    '/review-ai on the new model adapter.',
  ],
  [
    'scraping-applier-expert',
    'critic',
    'Reviews scraping / browser automation, selector resilience, registry + workflow reliability.',
    'audits scraping-applier-author',
    'scraping/** · browser/** · SCRAPERS registry',
    '/review-scraping on the LinkedIn selector fix.',
  ],
  [
    'resume-export-expert',
    'critic',
    'Reviews the resume/export domain — DocumentModel, templates, theme, locale, ATS-safe structure.',
    'audits pdf-docx-generator',
    'export/** · model/** · theme/** · locale/** · layout/**',
    '/review-export on the pagination fix.',
  ],
  [
    'testing-reviewer',
    'critic',
    'Audits coverage of changed code + test quality — weak assertions, flakiness, untested error paths. Never writes tests.',
    'audits test-author',
    '**/*.test.* · **/*.spec.* · src-tauri/tests/**',
    'Use testing-reviewer to audit the new golden tests.',
  ],
  [
    'code-quality-reviewer',
    'critic',
    'Audits clean-code / DRY / KISS — severity-graded report, read-only.',
    'audits code-quality-author',
    'any package/path on request',
    '/code-quality-review on the scoring module.',
  ],
  [
    'tauri-security-reviewer',
    'critic',
    'The cross-cutting SECURITY AUTHORITY — IPC surface, capabilities, updater, net, credentials, supply-chain, AI injection.',
    'default Secondary on any risk-bearing change',
    'capabilities/ · net/ · credentials/ · commands/** · Cargo.* · package.json',
    '/review-security on the new IPC command.',
  ],
  [
    'performance-profiler',
    'critic',
    'The performance lens — startup, memory, hot paths, rendering, AI token efficiency. Secondary on perf-sensitive paths.',
    'Secondary alongside the domain Primary',
    'export/** · scraping/** · ai_provider/** · layout/** · measure/**',
    '/review-performance on the export hot path.',
  ],
  [
    'extension-reviewer',
    'critic',
    'Reviews the browser extension + bridge — MV3 compliance, permission minimization, per-frame token auth correctness, protocol lockstep (TS ↔ Rust), and Chrome Web Store + Firefox AMO store-policy compliance.',
    'audits extension-author (+ tauri-security-reviewer on auth/permission/data risk)',
    'apps/extension/** · extension_bridge/** · extension-protocol*',
    '/review-extension on the new message type.',
  ],
  [
    'webgl-reviewer',
    'critic',
    'Read-only last-line critic over the diffs from both GL authors — webgl-author (scenes/engine) and shader-engineer (GLSL/post) — checking scrub-safety, resource disposal, per-frame allocation, uniform-vs-recompile correctness, draw-call budgets, semantic-layer parity, and gate integrity.',
    'audits webgl-author + shader-engineer',
    'apps/landing/src/**',
    '/review-webgl on the new landing scene.',
  ],
];

export const CROSS: readonly AgentTuple[] = [
  [
    'finding-verifier',
    'cross',
    'Per-finding verification judge in the /review pipeline — reads code at file:line, refutes-by-default, requires rule-based claims to quote the exact rule or score 0. Never edits, only scores 0–100.',
    'runs per single-source candidate finding in /review synthesis',
    'review-config.md · .claude/agents/** · target file:line (read-only)',
    '/review to score findings <80 confidence.',
  ],
  [
    'cleanup',
    'cross',
    'Dead-code audit across the TS/React + Rust monorepo — unused files, exports, deps. Report-first; safe-tier deletes only after confirmation.',
    'runs immediately before project-steward',
    'whole monorepo',
    '/cleanup after the feature lands.',
  ],
  [
    'project-steward',
    'cross',
    'Sole owner of docs, the knowledge base, ADRs, the lessons log, and release — the only agent that persists lessons.',
    'closes every task',
    'docs/** · docs/knowledge/** · landing/** · release config',
    '/update-docs to sync docs + graphify after the change.',
  ],
  [
    'pr-reviewer',
    'cross',
    'Strict generalist PRE-PR reviewer — runs the real repo tools (typecheck, lint, clippy, tests) + cross-file blast-radius + a verification gate before a PR opens, so CodeRabbit finds fewer issues.',
    'final internal gate before push — 🔴+🟠 block',
    'any changed files (diff-scoped)',
    '/review before opening a PR.',
  ],
  [
    'gate-auditor',
    'cross',
    'Rendered-output auditor — drives the dev server via Chrome DevTools MCP to exact playhead positions, screenshots, traces, and console; runs the milestone gates, scrub determinism, draw-call probe, strobe budget, and copy parity. Never edits code.',
    'runs the visual gate on apps/landing rendered-output changes',
    'apps/landing (dev server on :3000)',
    '/gate on the current landing milestone.',
  ],
  [
    'webgl-perf-profiler',
    'cross',
    'GL frame-rate lens for apps/landing — traces the worst scroll segments, then applies the webgl-standards degradation ladder in order (pixel ratio, post samples, geometry density, effect toggles), stopping at the first rung that passes. Distinct from performance-profiler.',
    'Secondary on apps/landing GL frame rate',
    'apps/landing/src/** (GL frame loop)',
    'Use webgl-perf-profiler when a landing scroll segment drops below target FPS.',
  ],
];

// Author → its independent critic(s). A critic shared by two authors
// (webgl-reviewer) is listed under each; the map renders it once.
export const PAIRS: readonly (readonly [string, readonly string[]])[] = [
  ['rust-backend-author', ['rust-backend-architect']],
  ['frontend-author', ['frontend-reviewer', 'ui-ux-expert']],
  ['job-match-author', ['job-match-expert']],
  ['ai-provider-author', ['ai-provider-expert']],
  ['scraping-applier-author', ['scraping-applier-expert']],
  ['pdf-docx-generator', ['resume-export-expert']],
  ['test-author', ['testing-reviewer']],
  ['code-quality-author', ['code-quality-reviewer']],
  ['extension-author', ['extension-reviewer']],
  ['webgl-author', ['webgl-reviewer']],
  ['shader-engineer', ['webgl-reviewer']],
];

// Cross-cutting / risk agents — they ride along, no author pairing.
export const CROSS_NODES: readonly string[] = [
  'finding-verifier',
  'tauri-security-reviewer',
  'performance-profiler',
  'webgl-perf-profiler',
  'gate-auditor',
  'cleanup',
  'project-steward',
  'pr-reviewer',
];

export const BY_NAME: ReadonlyMap<string, AgentTuple> = new Map(
  [...AUTHORS, ...CRITICS, ...CROSS].map((tuple) => [tuple[0], tuple])
);

// ── Intake → delegation routing (the "one issue in" demo) ────────────────────
type RouteRow =
  | { kind: 'area'; detail: string }
  | { kind: 'author' | 'critic' | 'secondary' | 'gate'; name: string; why: string };

export interface RouteCase {
  id: string;
  issue: string;
  title: string;
  area: string;
  rows: readonly RouteRow[];
}

const GATE_ROW: RouteRow = {
  kind: 'gate',
  name: 'pr-reviewer',
  why: 'final pre-PR gate — runs the real tools + blast-radius before the PR (/review)',
};

export const ROUTES: readonly RouteCase[] = [
  {
    id: 'focus',
    issue: 'Button has no focus ring',
    title: 'Button has no focus ring',
    area: 'apps/desktop/src/renderer/** (or packages/ui/**) → frontend',
    rows: [
      { kind: 'area', detail: 'UI / renderer code — a focus-ring is a renderer + a11y concern.' },
      {
        kind: 'author',
        name: 'frontend-author',
        why: 'adds the focus style on the @ajh/ui primitive',
      },
      { kind: 'critic', name: 'frontend-reviewer', why: 'checks ports-&-adapters + design tokens' },
      {
        kind: 'critic',
        name: 'ui-ux-expert',
        why: 'the a11y / visual taste lens — is the ring actually visible?',
      },
      GATE_ROW,
    ],
  },
  {
    id: 'ats',
    issue: 'ATS score seems wrong',
    title: 'ATS score seems wrong',
    area: 'commands/match_resume.rs, validate/, recommend/ → ATS scoring',
    rows: [
      {
        kind: 'area',
        detail: 'Scoring / matching logic — owned by the job-match domain, not export formatting.',
      },
      {
        kind: 'author',
        name: 'job-match-author',
        why: 'fixes the scoring kernel / keyword extraction',
      },
      {
        kind: 'critic',
        name: 'job-match-expert',
        why: 'audits match quality + recommendation correctness',
      },
      GATE_ROW,
    ],
  },
  {
    id: 'ipc',
    issue: 'Add an IPC command for X',
    title: 'Add an IPC command for X',
    area: 'commands/**, commands/mod.rs, packages/shared/** → backend + IPC',
    rows: [
      { kind: 'area', detail: 'New IPC surface — Rust backend, and a new attack surface.' },
      {
        kind: 'author',
        name: 'rust-backend-author',
        why: 'implements the command + wires the contract',
      },
      {
        kind: 'critic',
        name: 'rust-backend-architect',
        why: 'reviews boundaries, errors, data integrity',
      },
      {
        kind: 'secondary',
        name: 'tauri-security-reviewer',
        why: 'default risk Secondary — every new command is IPC attack surface',
      },
      GATE_ROW,
    ],
  },
  {
    id: 'pdf',
    issue: 'PDF export overflows a page',
    title: 'PDF export overflows a page',
    area: 'export/**, layout/**, measure/** → resume/export',
    rows: [
      {
        kind: 'area',
        detail: 'Export rendering + pagination — the resume/export domain (perf-sensitive).',
      },
      {
        kind: 'author',
        name: 'pdf-docx-generator',
        why: 'fixes layout / pagination in the renderer',
      },
      {
        kind: 'critic',
        name: 'resume-export-expert',
        why: 'audits ATS-safe structure + template correctness',
      },
      {
        kind: 'secondary',
        name: 'performance-profiler',
        why: 'export is a hot path — perf lens rides along',
      },
      GATE_ROW,
    ],
  },
  {
    id: 'scrape',
    issue: 'Scraper stopped matching LinkedIn',
    title: 'Scraper stopped matching LinkedIn',
    area: 'scraping/**, browser/**, SCRAPERS registry → scraping',
    rows: [
      { kind: 'area', detail: 'Selector resilience + browser automation — the scraping domain.' },
      {
        kind: 'author',
        name: 'scraping-applier-author',
        why: 'repairs the LinkedIn selectors in the registry',
      },
      {
        kind: 'critic',
        name: 'scraping-applier-expert',
        why: 'audits selector resilience + workflow reliability',
      },
      GATE_ROW,
    ],
  },
];

// ── The assembly line (nine stations) ────────────────────────────────────────
export type MachineKind = 'router' | 'pen' | 'mag' | 'tube' | 'broom' | 'quill' | 'gate' | 'rocket';

export interface Station {
  title: string;
  access: string;
  desc: string;
  agentTag: string;
  machine: MachineKind;
  stamp: string;
}

export const STATIONS: readonly Station[] = [
  {
    title: 'intake & triage',
    access: 'main session',
    desc: 'paths matched to review-routes.json; first glob wins.',
    agentTag: '',
    machine: 'router',
    stamp: '·',
  },
  {
    title: 'author implements',
    access: 'write access',
    desc: 'the domain author makes the smallest diff that fits.',
    agentTag: 'rust-backend-author',
    machine: 'pen',
    stamp: '✎',
  },
  {
    title: 'critic audits',
    access: 'read-only',
    desc: 'an independent critic reviews the diff. it never wrote it.',
    agentTag: 'rust-backend-architect',
    machine: 'mag',
    stamp: '✓',
  },
  {
    title: 'test-author',
    access: 'if testable',
    desc: 'adds unit / golden / e2e coverage for the change.',
    agentTag: 'test-author',
    machine: 'tube',
    stamp: '🧪',
  },
  {
    title: 'testing-reviewer',
    access: 'read-only',
    desc: 'challenges weak assertions + untested error paths.',
    agentTag: 'testing-reviewer',
    machine: 'mag',
    stamp: '✓✓',
  },
  {
    title: 'cleanup',
    access: 'report-first',
    desc: 'sweeps dead code the change orphaned; safe deletes only.',
    agentTag: 'cleanup',
    machine: 'broom',
    stamp: '✦',
  },
  {
    title: 'project-steward',
    access: 'sole doc writer',
    desc: 'syncs docs/knowledge, persists lessons, updates the graphs.',
    agentTag: 'project-steward',
    machine: 'quill',
    stamp: '📖',
  },
  {
    title: 'pr-reviewer gate',
    access: 'before the PR',
    desc: 'real tools + blast-radius. 🔴+🟠 block the PR.',
    agentTag: 'pr-reviewer',
    machine: 'gate',
    stamp: '🛡',
  },
  {
    title: 'ship',
    access: 'PR opens',
    desc: 'CodeRabbit finds less, because the fleet already did.',
    agentTag: '',
    machine: 'rocket',
    stamp: '🚀',
  },
];

export const AGENT_COUNT = AUTHORS.length + CRITICS.length + CROSS.length;
