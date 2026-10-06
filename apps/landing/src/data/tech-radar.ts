// Single source of truth for the /tech-radar page — a curated, human-judged
// list (not derived from package.json, deliberately: the whole point of a
// radar is the judgment call, same as a changelog or an ADR). Two things keep
// a curated list honest instead of letting it rot silently:
//
//   1. scripts/check-tech-radar.mjs (CI, wired into ci-pipeline.yml + the
//      pre-push hook) fails the build when a `subjectKind: 'dependency'`
//      entry names a package that no longer exists in any package.json /
//      Cargo.toml on disk — update the entry or remove it.
//   2. `adrSlug` is checked against docs/knowledge/decision-records/ the same way — a dead link
//      fails loudly instead of quietly pointing nowhere.
//
// Every entry object (in ./tech-radar/entries-*.ts) is intentionally FLAT (no nested objects/arrays)
// so the checker can bound one entry with a simple non-nested-brace regex —
// same convention as the node blocks in ./architecture-map.ts (see that
// file's header comment and scripts/check-landing-drift.mjs's
// CONTRACT_BLOCK_RE for why that convention exists).
//
// Rings follow the standard tech-radar convention (innermost = most settled):
// Adopt → Trial → Assess → Hold. Quadrants are named for THIS project's real
// shape rather than Thoughtworks' generic set — see QUADRANTS below.

import { backendDataEntries } from './tech-radar/entries-backend-data';
import { buildShipTrustEntries } from './tech-radar/entries-build-ship-trust';
import { documentsExportEntries } from './tech-radar/entries-documents-export';
import { rendererUiEntries } from './tech-radar/entries-renderer-ui';
import type { RadarQuadrant, RadarRing, TechRadarEntry } from './tech-radar/types';

export type {
  RadarQuadrant,
  RadarRing,
  RadarSubjectKind,
  TechRadarEntry,
} from './tech-radar/types';

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
  ...rendererUiEntries,
  ...backendDataEntries,
  ...documentsExportEntries,
  ...buildShipTrustEntries,
];
