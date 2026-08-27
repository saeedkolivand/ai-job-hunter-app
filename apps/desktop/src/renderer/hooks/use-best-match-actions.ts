import { useCallback, useState } from 'react';

import type { AutopilotBestMatch } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { useNotification } from '@ajh/ui';

import { useApplyToFoundJob } from '@/hooks/use-apply-to-found-job';
import { useAutopilots, useOpenExternal, usePersistJob } from '@/services';

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
 * call, this only cancels the OPTIMISTIC hide.
 *
 * `handleDismiss` deliberately does NOT invalidate `keys.autopilot.bestMatches`
 * (or its parent `keys.autopilot.all`) on success. The dismiss write is a
 * local JSON persist — it resolves in milliseconds, long before a user could
 * ever reach for Undo — so forcing a refetch there would immediately drop the
 * row out of the query cache (`compute_best_matches` already excludes
 * dismissed jobs server-side) and make Undo a dead button: it would delete a
 * key from `dismissedKeys` for a row that no longer exists anywhere to show.
 * An affordance that renders but does nothing is worse than not shipping one.
 * The optimistic hide is enough on its own to keep the row gone for the rest
 * of this mount; the backend is already authoritative, so any NATURAL
 * refetch this hook doesn't control (a remount, a route change, another
 * autopilot mutation invalidating `keys.autopilot.all`) will reflect the
 * dismissal on its own once it happens — local state and the backend only
 * ever disagree about *when* the row leaves, never about *whether* it does.
 * `handleView`/`handleSave` were checked against the same question
 * deliberately: neither participates in `compute_best_matches`'s
 * qualification predicate (only autopilot membership + dismissal do), so
 * there is nothing for either to invalidate — confirmed by their own tests.
 */
export function useBestMatchActions() {
  const { t } = useTranslation();
  const notify = useNotification();
  const openExternal = useOpenExternal();
  const persistJob = usePersistJob();
  const applyToFoundJob = useApplyToFoundJob();
  const { data: autopilots } = useAutopilots();
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
        // No onSuccess invalidation — see the hook's doc comment above for why
        // that would make Undo inert.
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
    [persistJob, notify, t]
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
