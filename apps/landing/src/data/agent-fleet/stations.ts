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
