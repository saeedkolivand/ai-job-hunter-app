import { useState } from 'react';
import { useNavigate } from '@tanstack/react-router';

import { useTranslation } from '@ajh/translations';
import { useNotification } from '@ajh/ui';

import { useRowMatchScore } from '@/features/jobs/providers';
import { scoreToLevel } from '@/lib/match-level';
import { useOpenExternal, usePersistJob } from '@/services';
import { useSaveFromPosting } from '@/services/use-applications';
import { useSessionStore } from '@/store/session-store';

import type { Posting } from '../types';

/**
 * Extracts the non-JSX action logic from PostingRow so PostingRow, PostingListItem
 * and JobDetailPane can all reuse the same persist/track/navigate implementation.
 */
export function usePostingActions(posting: Posting) {
  const { t } = useTranslation();
  const notify = useNotification();
  const navigate = useNavigate();
  const setApplicationApply = useSessionStore((s) => s.setApplicationApply);
  const openExternalMutation = useOpenExternal();
  const persistJobMutation = usePersistJob();
  const saveFromPostingMutation = useSaveFromPosting();
  const { score } = useRowMatchScore(posting.id);

  const [interactionTypes, setInteractionTypes] = useState(
    () => new Set<string>(posting.interactions?.map((i) => i.interactionType) ?? [])
  );

  const jobPayload = {
    id: posting.id,
    source: posting.source,
    externalId: posting.externalId,
    url: posting.url,
    title: posting.title,
    company: posting.company,
    location: posting.location,
    description: posting.description,
    capturedAt: posting.capturedAt,
  };

  const trackInteraction = async (
    interactionType: 'viewed' | 'opened' | 'applied' | 'bookmarked'
  ) => {
    setInteractionTypes((prev) => new Set([...prev, interactionType]));
    try {
      await persistJobMutation.mutateAsync({ job: jobPayload, interactionType });
    } catch (err) {
      console.error('Failed to track interaction:', err);
    }
  };

  const has = (type: string) => interactionTypes.has(type);

  const handleOpen = () => {
    void trackInteraction('opened');
    void openExternalMutation.mutateAsync(posting.url);
  };

  const handleCopyLink = async () => {
    try {
      await navigator.clipboard.writeText(posting.url);
      notify.success({ message: t('jobs.copyLink') });
    } catch {
      notify.error({ message: t('jobs.copyLinkError') });
    }
  };

  const handleTailor = async () => {
    let res;
    try {
      res = await saveFromPostingMutation.mutateAsync({
        jobUrl: posting.url,
        board: posting.source,
        company: posting.company,
        title: posting.title,
        jobDescription: posting.description,
        salaryMin: posting.salaryMin,
        salaryMax: posting.salaryMax,
        salaryCurrency: posting.salaryCurrency,
      });
    } catch {
      notify.error({ message: t('jobs.tailorError') });
      return;
    }
    if (!res?.id) {
      notify.error({ message: t('jobs.tailorError') });
      return;
    }
    // Mark `applied` only after the save succeeds — trackInteraction persists via
    // persistJobMutation and the failure paths above don't roll back. Mirrors handleSave.
    void trackInteraction('applied');
    setApplicationApply({
      applyForId: res.id,
      applySeedResume: null,
      applyMatchLevel: typeof score?.combined === 'number' ? scoreToLevel(score.combined) : null,
      applyWizardStep: 0,
      applyWizardForm: null,
    });
    void navigate({
      to: '/applications/$id',
      params: { id: res.id },
      search: { tab: 'documents', from: 'jobs' },
    });
  };

  const handleView = () => void navigate({ to: '/applications' });

  const handleSave = () => {
    void saveFromPostingMutation
      .mutateAsync({
        jobUrl: posting.url,
        board: posting.source,
        company: posting.company,
        title: posting.title,
        jobDescription: posting.description,
        salaryMin: posting.salaryMin,
        salaryMax: posting.salaryMax,
        salaryCurrency: posting.salaryCurrency,
      })
      .then(() => {
        // Only mark saved and notify AFTER the mutation resolves successfully.
        void trackInteraction('bookmarked');
        notify.success({ message: t('applications.savedToTracking') });
      })
      .catch(() => {
        notify.error({ message: t('jobs.saveError') });
      });
  };

  const saved = interactionTypes.has('bookmarked');
  const pending = saveFromPostingMutation.isPending;

  return {
    has,
    interactionTypes,
    trackInteraction,
    handleOpen,
    handleCopyLink,
    handleTailor,
    handleView,
    handleSave,
    saved,
    pending,
    score,
  };
}
