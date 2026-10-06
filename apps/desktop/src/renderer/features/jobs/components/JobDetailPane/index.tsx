import { Briefcase, Loader2, RefreshCw } from 'lucide-react';
import { motion } from 'motion/react';
import { useEffect, useRef } from 'react';

import { useTranslation } from '@ajh/translations';
import {
  Button,
  EmptyState,
  JobDescription,
  resolveTransition,
  transition,
  variants,
} from '@ajh/ui';

import { usePostingActions } from '@/features/jobs/hooks/usePostingActions';
import type { Posting } from '@/features/jobs/types';

import { ClusterSources } from './ClusterSources';
import { DetailHeader } from './DetailHeader';
import { useResolvedDescription } from './useResolvedDescription';

// Dwell threshold before a job is marked as viewed (5s per spec).
const VIEWED_DWELL_MS = 5000;

interface JobDetailPaneProps {
  posting: Posting | null;
  formatRelativeTime: (timestamp?: number) => string;
}

function DetailContent({
  posting,
  formatRelativeTime,
}: {
  posting: Posting;
  formatRelativeTime: (timestamp?: number) => string;
}) {
  const { t } = useTranslation();
  const actions = usePostingActions(posting);
  const { trackInteraction } = actions;
  const { description, descLoading, showLoadButton, showError, refetch, announced } =
    useResolvedDescription(posting);

  // Mark 'viewed' after a 5s dwell (fire-once per job mount via key={posting.id}).
  // Depends ONLY on posting.id so a description-resolve re-render can't reset/refire it.
  // clearTimeout in cleanup cancels on job-switch or unmount.
  const trackInteractionRef = useRef(trackInteraction);
  trackInteractionRef.current = trackInteraction;
  const viewedFiredRef = useRef(false);
  useEffect(() => {
    const id = setTimeout(() => {
      if (!viewedFiredRef.current) {
        viewedFiredRef.current = true;
        void trackInteractionRef.current('viewed');
      }
    }, VIEWED_DWELL_MS);
    return () => clearTimeout(id);
  }, [posting.id]);

  // Reduced-motion: keep opacity fade but drop the y-translate (no positional jump).
  const resolvedTransition = resolveTransition(transition.fast);
  const isInstant = resolvedTransition.duration === 0;

  return (
    <motion.div
      initial={isInstant ? { opacity: 0 } : variants.fadeSlideUp.initial}
      animate={isInstant ? { opacity: 1 } : variants.fadeSlideUp.animate}
      exit={isInstant ? { opacity: 0 } : variants.fadeSlideUp.exit}
      transition={resolvedTransition}
      className="flex h-full flex-col overflow-hidden"
    >
      <DetailHeader posting={posting} formatRelativeTime={formatRelativeTime} actions={actions} />

      {/* Body — description.
          The live region is a small visually-hidden sentinel only (blocker 4):
          it announces the single "full description loaded" message once on
          the snippet→full upgrade, without noisily re-announcing the whole body. */}
      <div className="min-h-0 flex-1 overflow-y-auto px-5 pb-4 pt-1">
        {/* Visually-hidden AT sentinel — announces once when description upgrades */}
        <span role="status" aria-live="polite" aria-atomic="true" className="sr-only">
          {announced ? t('jobs.fullDescriptionLoaded') : ''}
        </span>

        <ClusterSources posting={posting} />

        {/* "About the job" section label */}
        <h3 className="mb-3 text-fine-print uppercase tracking-wider text-muted-foreground">
          {t('jobs.aboutTheJob')}
        </h3>

        {/* Loading state: only shown when there is NO text to display yet */}
        {descLoading && !description && (
          <div
            role="status"
            aria-busy="true"
            className="flex items-center gap-2 text-sm text-foreground/70"
          >
            <Loader2 size={14} aria-hidden="true" className="animate-spin" />
            {t('jobs.loadingDescription')}
          </div>
        )}

        {/* Inline updating hint: text exists but we're still fetching a longer version */}
        {descLoading && description && (
          <p
            className="mb-2 flex items-center gap-1.5 text-[10px] text-foreground/40"
            aria-hidden="true"
          >
            <Loader2 size={10} aria-hidden="true" className="animate-spin" />
            {t('jobs.updatingDescription')}
          </p>
        )}

        {/* Description — rendered immediately when any text is available */}
        {description && (
          <>
            {/* fold 9: space-y-4 for block rhythm; headings use mt-2 not mt-4 */}
            <JobDescription
              markdown={description}
              className="space-y-4 text-caption text-foreground/80"
            />

            {/* blocker 7: show error hint when resolve failed AND the gate fired;
                gate on shouldResolve so non-aggregator postings are never affected */}
            {showError && (
              <p className="mt-2 text-[11px] text-foreground/50">
                {t('jobs.descriptionLoadError')}
              </p>
            )}

            {/* Load button is OUTSIDE the live region (blocker 4) */}
            {showLoadButton && (
              <Button
                variant="ghost"
                onClick={() => void refetch()}
                className="mt-2 h-auto w-fit gap-1 px-2 py-1 text-[11px] text-foreground/50 hover:text-foreground/80"
              >
                <RefreshCw size={11} aria-hidden="true" />
                {t('jobs.loadFullDescription')}
              </Button>
            )}
          </>
        )}
      </div>
    </motion.div>
  );
}

export function JobDetailPane({ posting, formatRelativeTime }: JobDetailPaneProps) {
  const { t } = useTranslation();

  if (!posting) {
    return (
      <div className="flex h-full items-center justify-center">
        <EmptyState icon={Briefcase} title={t('jobs.selectAJob')} className="py-10" />
      </div>
    );
  }

  // key={posting.id} remounts DetailContent per job so usePostingActions'
  // lazy useState initializer re-seeds from the new posting's interactions.
  // Without this, switching jobs in split view leaks A's saved/viewed state into B.
  return (
    <DetailContent key={posting.id} posting={posting} formatRelativeTime={formatRelativeTime} />
  );
}
