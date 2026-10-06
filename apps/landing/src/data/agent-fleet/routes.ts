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
