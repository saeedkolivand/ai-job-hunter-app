import type { Application } from '@ajh/shared';

import { nextActionLabel } from './stale';

/**
 * The six columns of the Applications pipeline strip. Five map 1:1 onto a
 * lifecycle stage; `closed` aggregates the four end-states so the strip stays
 * six cards wide however a pursuit finished.
 *
 * Derived-by-hand rather than from `APPLICATION_STAGES` on purpose: the grouping
 * is a UI decision (what the user wants to see side by side), not a domain one.
 * `pipeline.test.ts` pins it against `APPLICATION_STAGES` so a new backend stage
 * can never silently fall out of the strip.
 */
export const PIPELINE_GROUPS = [
  { id: 'saved', stages: ['saved'] },
  { id: 'applied', stages: ['applied'] },
  { id: 'screening', stages: ['screening'] },
  { id: 'interviewing', stages: ['interviewing'] },
  { id: 'offer', stages: ['offer'] },
  { id: 'closed', stages: ['accepted', 'rejected', 'ghosted', 'withdrawn'] },
] as const;

export type PipelineGroupId = (typeof PIPELINE_GROUPS)[number]['id'];

const STAGES_BY_GROUP = new Map<string, readonly string[]>(
  PIPELINE_GROUPS.map((g) => [g.id, g.stages])
);

/**
 * The stage ids a group covers, or `null` for no/unknown group — callers treat
 * `null` as "no stage filter active", so a stale group id in session state
 * degrades to the unfiltered list instead of an empty page.
 */
export function stagesForGroup(groupId: string | null | undefined): readonly string[] | null {
  if (!groupId) return null;
  return STAGES_BY_GROUP.get(groupId) ?? null;
}

/**
 * Per-group counts. Always carries an entry for EVERY group (zeros included) so
 * the strip can render all six cards unconditionally.
 */
export function pipelineCounts(
  applications: readonly Application[]
): Record<PipelineGroupId, number> {
  const counts = Object.fromEntries(PIPELINE_GROUPS.map((g) => [g.id, 0])) as Record<
    PipelineGroupId,
    number
  >;
  for (const app of applications) {
    const group = PIPELINE_GROUPS.find((g) => (g.stages as readonly string[]).includes(app.status));
    if (group) counts[group.id] += 1;
  }
  return counts;
}

/**
 * Applications whose follow-up reminder has passed. Derived CLIENT-SIDE from the
 * `nextActionAt` already carried by `applications.list()` — deliberately no new
 * IPC command for a count the list payload can answer.
 */
export function overdueCount(applications: readonly Application[]): number {
  return applications.filter((a) => nextActionLabel(a.nextActionAt) === 'overdue').length;
}

/** List sort orders offered in the page header. `updated` mirrors the server order. */
export const APPLICATION_SORTS = ['updated', 'company', 'nextAction'] as const;
export type ApplicationSort = (typeof APPLICATION_SORTS)[number];

/** Narrow an arbitrary persisted value back to a known sort (defaults to `updated`). */
export function toApplicationSort(value: string | null | undefined): ApplicationSort {
  return (APPLICATION_SORTS as readonly string[]).includes(value ?? '')
    ? (value as ApplicationSort)
    : 'updated';
}

/**
 * Non-mutating sort of an application list.
 *
 * - `updated` — most recently touched first (the server's own order).
 * - `company` — A→Z by company, then title, so one company's pursuits sit together.
 * - `nextAction` — soonest reminder first; applications with NO reminder sink to
 *   the bottom (they need no attention), ordered by recency among themselves.
 */
export function sortApplications(
  applications: readonly Application[],
  sort: ApplicationSort
): Application[] {
  const out = [...applications];
  switch (sort) {
    case 'company':
      return out.sort(
        (a, b) => a.company.localeCompare(b.company) || a.title.localeCompare(b.title)
      );
    case 'nextAction':
      return out.sort(
        (a, b) =>
          (a.nextActionAt ?? Number.POSITIVE_INFINITY) -
            (b.nextActionAt ?? Number.POSITIVE_INFINITY) || b.updatedAt - a.updatedAt
      );
    case 'updated':
    default:
      return out.sort((a, b) => b.updatedAt - a.updatedAt);
  }
}
