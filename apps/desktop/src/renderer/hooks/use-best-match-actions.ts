import { useCallback, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import type { AutopilotBestMatch } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { useNotification } from '@ajh/ui';

import { useApplyToFoundJob } from '@/hooks/use-apply-to-found-job';
import { keys, useAutopilots, useOpenExternal, usePersistJob } from '@/services';

/**
 * Interaction payload for a best-match row. Deliberately carries `id`
 * (mirrors the `AutopilotCard.handleJobClick` fix — `InteractionStore::upsert`
 * keys on `(job_id, interaction_type)`, and `id` defaults to `""` server-side
 * without it) AND, for `dismissed` specifically, `title` + `company`: Rust's
 * `compute_best_matches` matches a dismissal by deriving
 * `canonical_job_key(url, title, company)` against every cluster member's own
 * key — omitting either field means the derived key never matches and the
 * row never disappears (see `.claude/scratch/best-matches.md`'s Phase 1
 * outcome).
 */
function toPersistPayload(match: AutopilotBestMatch, interactionType: string) {
  return {
    job: {
      id: match.url,
      url: match.url,
      title: match.title,
      company: match.company,
      location: match.location ?? '',
      source: 'autopilot',
      externalId: match.url,
      description: '',
      capturedAt: Date.now(),
    },
    interactionType,
  };
}

/**
 * Shared row actions for a best-match — View / Save / Apply / Dismiss — used
 * by both `BestMatchesPreview` and `BestMatchesPage` so the two surfaces
 * share exactly one implementation (same reasoning as `useApplyToFoundJob`).
 *
 * `dismissedKeys` backs the "Dismissed — Undo" inline row: dismiss hides the
 * row optimistically (`onMutate`-style, matching `useRemoveAutopilot`'s own
 * optimistic-delete precedent) and rolls back on failure; `undoDismiss` just
 * reveals the row again locally — there is no server-side "un-dismiss" IPC
 * call, this only cancels the OPTIMISTIC hide. Once the dismiss mutation's
 * own `keys.autopilot.all` invalidation lands, a genuinely-dismissed row
 * stops coming back from the backend regardless of local state.
 */
export function useBestMatchActions() {
  const { t } = useTranslation();
  const notify = useNotification();
  const openExternal = useOpenExternal();
  const persistJob = usePersistJob();
  const applyToFoundJob = useApplyToFoundJob();
  const { data: autopilots } = useAutopilots();
  const qc = useQueryClient();
  const [dismissedKeys, setDismissedKeys] = useState<Set<string>>(new Set());

  const handleView = useCallback(
    (match: AutopilotBestMatch) => {
      void openExternal.mutate(match.url);
      persistJob.mutate(toPersistPayload(match, 'viewed'));
    },
    [openExternal, persistJob]
  );

  const handleSave = useCallback(
    (match: AutopilotBestMatch) => {
      persistJob.mutate(toPersistPayload(match, 'bookmarked'));
    },
    [persistJob]
  );

  const handleApply = useCallback(
    (match: AutopilotBestMatch) => {
      const sourceId = match.sources[0]?.autopilotId;
      const ap = autopilots?.find((a) => a._id === sourceId);
      if (!ap) return;
      void applyToFoundJob(match, ap);
    },
    [autopilots, applyToFoundJob]
  );

  const handleDismiss = useCallback(
    (match: AutopilotBestMatch) => {
      setDismissedKeys((prev) => new Set(prev).add(match.key));
      persistJob.mutate(toPersistPayload(match, 'dismissed'), {
        onSuccess: () => void qc.invalidateQueries({ queryKey: keys.autopilot.all }),
        onError: () => {
          setDismissedKeys((prev) => {
            const next = new Set(prev);
            next.delete(match.key);
            return next;
          });
          notify.error({ message: t('bestMatches.row.dismissFailed') });
        },
      });
    },
    [persistJob, qc, notify, t]
  );

  const undoDismiss = useCallback((key: string) => {
    setDismissedKeys((prev) => {
      const next = new Set(prev);
      next.delete(key);
      return next;
    });
  }, []);

  return { dismissedKeys, handleView, handleSave, handleApply, handleDismiss, undoDismiss };
}
