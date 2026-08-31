// Single source of truth for the /agent-system page (ported from the former
// hand-authored public/agent-system.html). Also the drift-guard EXPLAINER target
// for scripts/check-agent-system.mjs — every `.claude/agents/*.md` name must
// appear here (check 6), and the roster tuples below feed its reverse-check
// (check 9). Keep the `[name, role, …]` tuple shape so that guard keeps working.
//
// The GL fleet (webgl-author, shader-engineer, webgl-reviewer, gate-auditor,
// webgl-perf-profiler) is dormant — moved to .claude/dormant/, no GL surface
// since ADR-0017 — and intentionally absent from the roster below.

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
    'agent-cli-author',
    'author',
    'Implements the agent-facing CLI (`ajh-tauri agent …`) — verbs, argv parsing, the agent.query bridge resources, allowlist projections, and the machine-readable output contract an AI agent scripts against.',
    '↔ critic agent-cli-reviewer (+ tauri-security-reviewer on destructive or data-exposing verbs)',
    'apps/desktop/src-tauri/src/extension_bridge/agent_cli.rs · agent_read.rs',
    'Use agent-cli-author to add a new CLI verb.',
  ],
  [
    'extension-author',
    'author',
    'Implements the browser extension (MV3, Chrome + Firefox) and the desktop↔extension bridge (native-host + loopback WS, per-frame token auth) plus the shared wire protocol.',
    '↔ critic extension-reviewer (+ tauri-security-reviewer on auth/permission/data risk)',
    'apps/extension/** · apps/desktop/src-tauri/src/extension_bridge/** · packages/shared/src/ipc/extension-protocol*',
    'Use extension-author to add a new extension message type.',
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
    'agent-cli-reviewer',
    'critic',
    'Reviews the agent-facing CLI — nested projection passthroughs, output-contract stability, help/schema drift, throttle scope across fresh processes, and destructive-command ergonomics. Runs on a different model family from its author by design.',
    'audits agent-cli-author (+ tauri-security-reviewer on destructive or data-exposing verbs)',
    'agent_cli.rs · agent_read.rs · CLI verb tables',
    '/review-agent-cli on the new verb.',
  ],
  [
    'extension-reviewer',
    'critic',
    'Reviews the browser extension + bridge — MV3 compliance, permission minimization, per-frame token auth correctness, protocol lockstep (TS ↔ Rust), and Chrome Web Store + Firefox AMO store-policy compliance.',
    'audits extension-author (+ tauri-security-reviewer on auth/permission/data risk)',
    'apps/extension/** · extension_bridge/** · extension-protocol*',
    '/review-extension on the new message type.',
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
];

// Author → its independent critic(s). A critic shared by two authors is
// listed under each; the map renders it once.
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
  ['agent-cli-author', ['agent-cli-reviewer']],
];

// Cross-cutting / risk agents — they ride along, no author pairing.
export const CROSS_NODES: readonly string[] = [
  'finding-verifier',
  'tauri-security-reviewer',
  'performance-profiler',
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

// ── AgentFleet.tsx presentation data (additive — moved out of the component
//    in the PR-3 split so the component stays render-only) ──────────────────

// The hero illustration's SVG geometry — numeric attributes only; the shape
// types/classes/order stay in HeroSection.tsx.
export const HERO_SCENE = {
  viewBox: '0 0 560 220',
  authorBox: { x: 40, y: 70, width: 120, height: 92, rx: 14 },
  authorEyeL: { cx: 74, cy: 104, r: 8 },
  authorEyeR: { cx: 126, cy: 104, r: 8 },
  authorMouth: 'M78 134 q22 12 44 0',
  authorAntenna: 'M100 70 V52',
  authorAntennaTip: { cx: 100, cy: 48, r: 5 },
  authorArm: 'M150 120 l34 14',
  authorArrow: 'M182 128 l14 10 -10 6 z',
  diffCardBox: { x: 250, y: 92, width: 60, height: 74, rx: 7 },
  diffLine1: 'M262 112 h36',
  diffLine2: 'M262 126 h26',
  diffLine3: 'M262 140 h32',
  connector: 'M196 150 q90 30 150 -6',
  criticBox: { x: 400, y: 70, width: 120, height: 92, rx: 14 },
  criticBrow: 'M424 100 q12 -8 24 0',
  criticEyeL: { cx: 436, cy: 108, r: 7 },
  criticEyeR: { cx: 488, cy: 106, r: 11 },
  criticHandle: 'M496 114 l10 10',
  criticMouth: 'M428 140 q24 -8 48 0',
} as const;

// The six `<Divider>` squiggle paths, in page order.
export const DIVIDER_HERO = 'M2 9 Q160 2 320 8 T618 7';
export const DIVIDER_INTAKE = 'M2 8 Q160 14 320 7 T618 9';
export const DIVIDER_BELT = 'M2 9 Q160 3 320 9 T618 7';
export const DIVIDER_FLEET = 'M2 8 Q160 13 320 7 T618 9';
export const DIVIDER_TOGETHER = 'M2 9 Q160 2 320 8 T618 8';
export const DIVIDER_VERSUS = 'M2 8 Q160 13 320 7 T618 9';

// The Fleet section's color-key legend rows. Labels carry a leading space to
// match the single-text-node shape the hand-authored markup rendered.
export const FLEET_LEGEND: readonly { swatchClass: string; label: string }[] = [
  { swatchClass: 'sw author', label: ' author (writes)' },
  { swatchClass: 'sw critic', label: ' critic (audits)' },
  { swatchClass: 'sw cross', label: ' cross-cutting / risk' },
  { swatchClass: 'sw line', label: ' author ↔ critic pairing' },
  { swatchClass: 'sw line risk', label: ' risk secondary' },
];

// The "Work Together" cross-layer delegate prompt. The copied text and the
// displayed text differ only in quote style (straight vs curly-safe &quot;),
// so both variants share one phrase + body instead of duplicating the whole
// sentence.
const SAVE_JOB_PHRASE = 'save job to a folder';
const SAVE_JOB_PROMPT_BODY =
  'feature end-to-end: new IPC command + Rust store, a renderer panel to pick the folder, and ATS-aware tagging. Route it through the agent fleet.';
export const SAVE_JOB_PROMPT_COPY = `Implement a '${SAVE_JOB_PHRASE}' ${SAVE_JOB_PROMPT_BODY}`;
export const SAVE_JOB_PROMPT_TEXT = `Implement a "${SAVE_JOB_PHRASE}" ${SAVE_JOB_PROMPT_BODY}`;

// The three "Work Together" team cards. Each list item is a sequence of
// segments — `code` renders as <code>, `text` as plain text — so mixed
// inline-code + prose reproduces exactly without hardcoding JSX per item.
export type TeamSegment = { code: string } | { text: string };

export interface Team {
  title: string;
  items: readonly (readonly TeamSegment[])[];
}

export const WORK_TOGETHER_TEAMS: readonly Team[] = [
  {
    title: 'Disjoint files, in parallel',
    items: [
      [
        { code: 'rust-backend-author' },
        { text: ' — IPC command + SQLite store (' },
        { code: 'commands/' },
        { text: ', ' },
        { code: 'data_store.rs' },
        { text: ')' },
      ],
      [
        { code: 'frontend-author' },
        { text: ' — the folder-picker panel (' },
        { code: 'renderer/**' },
        { text: ')' },
      ],
      [{ code: 'job-match-author' }, { text: ' — the ATS tag logic' }],
    ],
  },
  {
    title: 'Critics challenge each other',
    items: [
      [
        { code: 'rust-backend-architect' },
        { text: ' + ' },
        { code: 'tauri-security-reviewer' },
        { text: ' on the new IPC surface' },
      ],
      [
        { code: 'frontend-reviewer' },
        { text: ' + ' },
        { code: 'ui-ux-expert' },
        { text: ' on the panel' },
      ],
      [{ code: 'job-match-expert' }, { text: ' on the scoring' }],
    ],
  },
  {
    title: 'Then the close-out',
    items: [
      [
        { code: 'test-author' },
        { text: ' → ' },
        { code: 'testing-reviewer' },
        { text: ' across all three' },
      ],
      [{ code: 'cleanup' }, { text: ' sweep' }],
      [{ code: 'project-steward' }, { text: ' docs + lessons' }],
    ],
  },
];
