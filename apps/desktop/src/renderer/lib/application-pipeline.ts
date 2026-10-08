import type { Application } from '@ajh/shared';

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
