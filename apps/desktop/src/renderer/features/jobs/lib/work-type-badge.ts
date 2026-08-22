import type { WorkTypeOption } from '@ajh/shared';

import type { Posting } from '../types';

const WORK_TYPE_BADGE_KEY: Record<WorkTypeOption, string> = {
  remote: 'jobs.remote',
  hybrid: 'jobs.hybrid',
  'on-site': 'jobs.onSite',
};

/**
 * i18n key for the green work-type badge (`PostingRow`, `JobDetailPane`), or
 * `null` when nothing should render. Prefers the declared `workType`; falls
 * back to the boolean `remote` flag for postings captured before this field
 * existed, or from a board that only ever exposed a boolean.
 */
export function workTypeBadgeKey(posting: Pick<Posting, 'workType' | 'remote'>): string | null {
  if (posting.workType) return WORK_TYPE_BADGE_KEY[posting.workType];
  return posting.remote ? 'jobs.remote' : null;
}
