// Single source of truth for the /agent-system page (ported from the former
// hand-authored public/agent-system.html). Also the drift-guard EXPLAINER target
// for scripts/check-agent-system.mjs — every `.claude/agents/*.md` name must
// appear here (check 6), and the roster tuples below feed its reverse-check
// (check 9). Keep the `[name, role, …]` tuple shape so that guard keeps working.
//
// The GL fleet (webgl-author, shader-engineer, webgl-reviewer, gate-auditor,
// webgl-perf-profiler) is dormant — moved to .claude/dormant/, no GL surface
// since ADR-0017 — and intentionally absent from the roster below.

import { type AgentRole, type AgentTuple, AUTHORS, CRITICS, CROSS } from './agent-fleet/roster';

export { AUTHORS, CRITICS, CROSS };
export type { AgentRole, AgentTuple };

export type { Team, TeamSegment } from './agent-fleet/presentation';
export {
  DIVIDER_BELT,
  DIVIDER_FLEET,
  DIVIDER_HERO,
  DIVIDER_INTAKE,
  DIVIDER_TOGETHER,
  DIVIDER_VERSUS,
  FLEET_LEGEND,
  HERO_SCENE,
  SAVE_JOB_PROMPT_COPY,
  SAVE_JOB_PROMPT_TEXT,
  WORK_TOGETHER_TEAMS,
} from './agent-fleet/presentation';
export type { RouteCase } from './agent-fleet/routes';
export { ROUTES } from './agent-fleet/routes';
export type { MachineKind, Station } from './agent-fleet/stations';
export { STATIONS } from './agent-fleet/stations';

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

export const AGENT_COUNT = AUTHORS.length + CRITICS.length + CROSS.length;
