import { useCallback } from 'react';
import { useNavigate } from '@tanstack/react-router';

import { AGGREGATOR_BOARD_ID, type Autopilot } from '@ajh/shared';

import { scoreToLevel } from '@/lib/match-level';
import { useSaveFromPosting } from '@/services';
import { useSessionStore } from '@/store/session-store';

/**
 * The minimal job shape the apply flow reads — satisfied structurally by both
 * `AutopilotFoundJob` (the per-card found-jobs list) and `AutopilotBestMatch`
 * (the cross-autopilot `/best-matches` list), so a caller in either surface
 * can pass its own row through unchanged.
 */
export interface ApplySourceJob {
  url: string;
  title: string;
  company: string;
  board?: string;
  salaryMin?: number;
  salaryMax?: number;
  salaryCurrency?: string;
  score?: number;
}

/**
 * Shared "Apply" action for a found/best-match job: creates (or reuses, deduped
 * by jobUrl) the Application, seeds the autopilot's résumé for the
 * Documents-tab wizard, then deep-links into the application detail —
 * `from=autopilot` makes its Back button return to whichever page opened it.
 *
 * Originally `AutopilotPage.handleApply` (#51) — lifted here so `/autopilot`
 * and `/best-matches` share exactly ONE implementation instead of two copies
 * that could drift. Errors and the "which autopilot/job to re-focus on Back"
 * bookkeeping both ride the shared `autopilot` session-store slice, same as
 * before the lift, so `AutopilotPage`'s existing error banner (driven by
 * `useAutopilotRun`'s `error`, itself just this same slice) keeps working
 * unchanged.
 */
export function useApplyToFoundJob() {
  const navigate = useNavigate();
  const saveFromPosting = useSaveFromPosting();
  const setAutopilot = useSessionStore((s) => s.setAutopilot);
  const setApplicationApply = useSessionStore((s) => s.setApplicationApply);

  return useCallback(
    async (job: ApplySourceJob, ap: Autopilot) => {
      try {
        const res = await saveFromPosting.mutateAsync({
          jobUrl: job.url,
          board: job.board ?? ap.target.boards[0] ?? AGGREGATOR_BOARD_ID,
          company: job.company,
          title: job.title,
          salaryMin: job.salaryMin,
          salaryMax: job.salaryMax,
          salaryCurrency: job.salaryCurrency,
        });
        if (!res?.id) {
          setAutopilot({ error: res?.error ?? 'Failed to create the application' });
          return;
        }
        setApplicationApply({
          applyForId: res.id,
          applySeedResume: ap.resumeText ?? null,
          applyMatchLevel: typeof job.score === 'number' ? scoreToLevel(job.score) : null,
          applyWizardStep: 0,
          applyWizardForm: null,
        });
        // Remember which autopilot (and specific job) we applied from so Back
        // re-expands it and scrolls to that row (consumed on the Autopilot
        // page's next mount).
        setAutopilot({ lastAppliedId: ap._id, lastAppliedJobUrl: job.url });
        void navigate({
          to: '/applications/$id',
          params: { id: res.id },
          search: { tab: 'documents', from: 'autopilot' },
        });
      } catch (e) {
        setAutopilot({
          error: e instanceof Error ? e.message : 'Failed to create the application',
        });
      }
    },
    [navigate, saveFromPosting, setAutopilot, setApplicationApply]
  );
}
