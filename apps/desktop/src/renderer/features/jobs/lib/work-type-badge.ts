import type { WorkTypeOption } from '@ajh/shared';
import type { TagColor } from '@ajh/ui';

import type { Posting } from '../types';

export interface WorkTypeBadge {
  /** i18n key for the badge label. */
  key: string;
  /**
   * Tag color. Green is reserved for "remote" elsewhere in the app's chip
   * vocabulary (purple = applied, blue = viewed, amber = saved), so on-site
   * and hybrid get their own tones rather than all three reusing green — a
   * user scanning a dense list would otherwise misread on-site as remote.
   */
  color: TagColor;
}

// `Map`, not a plain object index: a `WorkTypeOption` key never traverses the
// prototype chain, so an unexpected string (e.g. `'constructor'`) can never
// resolve to `Object.prototype` and be handed to `t()`.
const WORK_TYPE_BADGE = new Map<WorkTypeOption, WorkTypeBadge>([
  ['remote', { key: 'jobs.remote', color: 'green' }],
  ['hybrid', { key: 'jobs.hybrid', color: 'cyan' }],
  ['on-site', { key: 'jobs.onSite', color: 'default' }],
]);

/**
 * Badge shown on `PostingRow`/`JobDetailPane`, or `null` when nothing should
 * render. Prefers the declared `workType`; falls back to the boolean `remote`
 * flag for postings captured before this field existed, or from a board that
 * only ever exposed a boolean.
 */
export function getWorkTypeBadge(
  posting: Pick<Posting, 'workType' | 'remote'>
): WorkTypeBadge | null {
  if (posting.workType) return WORK_TYPE_BADGE.get(posting.workType) ?? null;
  return posting.remote ? { key: 'jobs.remote', color: 'green' } : null;
}
