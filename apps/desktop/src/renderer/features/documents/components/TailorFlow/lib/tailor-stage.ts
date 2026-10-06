import type { PipelineRunSummary } from '@ajh/shared/ipc';

import type { TailorRunState } from '../ResultsPanel';

export type TailorFlowStage = 'configuring' | 'generating' | 'done';

/**
 * A terminal `ResumePipelineState` → the results panel's status banner.
 *
 * A COLD entry (a past run redisplayed from `latestGeneration`, never
 * started/reconnected in THIS session) leaves the machine at `idle` — its
 * own state has no opinion, but this posting's run list (`runs`, already
 * fetched) does: `runs[0]` is that same latest run's real, persisted status.
 * Falling back to a blind `'done'` there rendered a needsReview or failed
 * run as a clean success. Any OTHER non-terminal state (queued/drafting/…)
 * still reads as `'done'` — those only happen with a live session, which
 * `GeneratingPanel` owns instead.
 */
export function toRunState(state: string, runs: PipelineRunSummary[]): TailorRunState {
  if (state === 'needsReview' || state === 'cancelled' || state === 'error') return state;
  if (state === 'idle') {
    const coldStatus = runs[0]?.status;
    if (coldStatus === 'needsReview') return 'needsReview';
    if (coldStatus === 'cancelled') return 'cancelled';
    if (coldStatus === 'failed') return 'error';
  }
  return 'done';
}
