import {
  Bookmark,
  Briefcase,
  CircleCheck,
  Copy,
  ExternalLink,
  Eye,
  Loader2,
  MapPin,
  RefreshCw,
  Save,
  Wand2,
} from 'lucide-react';
import { motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';

import { AGGREGATOR_BOARD_ID } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import {
  ActionMenu,
  Button,
  EmptyState,
  JobDescription,
  SourceBadge,
  Tag,
  transition,
  variants,
} from '@ajh/ui';

import { RowMatchScore } from '@/features/jobs/components/RowMatchScore';
import { usePostingActions } from '@/features/jobs/hooks/usePostingActions';
import { useMatchScores } from '@/features/jobs/providers';
import type { Posting } from '@/features/jobs/types';
import { useResolveJobUrl, useUpdatePostingDescription } from '@/services';

// ponytail: heuristic threshold — Adzuna search snippets are ~200–500 chars;
// anything under 700 chars for aggregator postings gets an on-demand resolve.
const SHORT_DESCRIPTION_CHARS = 700;

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
  const {
    has,
    trackInteraction,
    handleOpen,
    handleCopyLink,
    handleTailor,
    handleView,
    handleSave,
    saved,
    pending,
  } = usePostingActions(posting);

  // On-demand resolve gate:
  //   1. Always resolve when description is empty (original behaviour).
  //   2. Also resolve for aggregator (Adzuna) postings whose description is a
  //      short snippet below the threshold — the full text lives on the redirect URL.
  const descriptionEmpty = !posting.description?.trim();
  const snippetLen = posting.description?.trim().length ?? 0;
  const isAggregatorShort =
    posting.source === AGGREGATOR_BOARD_ID && snippetLen < SHORT_DESCRIPTION_CHARS;
  const shouldResolve = descriptionEmpty || isAggregatorShort;

  const resolved = useResolveJobUrl(posting.url, shouldResolve);

  // Keep-longer merge: never render text shorter than the original snippet.
  // The resolved text wins only when it is meaningfully longer (guards against
  // a 429 / generic-HTML result degrading the pane).
  const resolvedText = resolved.data?.description ?? '';
  const description: string = (() => {
    if (descriptionEmpty) return resolvedText;
    if (resolvedText.length > snippetLen) return resolvedText;
    return posting.description ?? '';
  })();

  // Gate both the loading indicator and the retry button off isFetching so any
  // refetch (including the manual retry click) consistently drives the UI.
  const descLoading = shouldResolve && resolved.isFetching;

  // Show the retry button when the description may still be incomplete:
  //  - gate fired (aggregator-short or empty), AND
  //  - not currently fetching, AND
  //  - resolved text is not yet meaningfully longer than the snippet.
  const resolvedLonger = resolvedText.length > snippetLen;
  const showLoadButton =
    (isAggregatorShort || descriptionEmpty) && !resolved.isFetching && !resolvedLonger;

  // Persist-then-score: one ordered one-shot effect so the backend always reads
  // the full markdown when computing the match score.
  //
  // Race that this fixes: on the render where resolve settles with longer text,
  // two independent effects could fire in the same commit — scoreJob's match.resume
  // might hit the backend BEFORE updateDescription persisted the full text, so the
  // score would be computed on the stale snippet while the pane shows the full text.
  //
  // Fix: single effect, single `doneRef` guard. When the resolve produced longer
  // text, persist FIRST then score inside `.finally()` (persist failure is non-fatal).
  // When no persist is needed (already-full description or resolve didn't improve),
  // score immediately. key={posting.id} on the parent resets the ref per job.
  const { scoreJob } = useMatchScores();
  const { mutateAsync: updateDescription } = useUpdatePostingDescription();
  const doneRef = useRef(false);
  useEffect(() => {
    if (doneRef.current) return;
    const descReady = description.trim().length > 0;
    // isFetched guards against the window before the query has started fetching —
    // without it resolveSettled could be true on the first render (isFetching=false,
    // data=undefined) and we'd score the snippet before the resolve even begins.
    const resolveSettled =
      !shouldResolve || (resolved.isFetched && !resolved.isFetching && !descLoading);
    if (!descReady || !resolveSettled) return;
    if (resolvedLonger) {
      // Persist the full text first, then score on it.
      // Latch only AFTER persist resolves so a transient IPC failure doesn't
      // permanently prevent a re-score with the full text.
      doneRef.current = true;
      void updateDescription({ id: posting.id, description })
        .catch(() => {
          // Persist failure is non-fatal — still score off the in-memory text,
          // but clear the latch so the pane can retry on next open.
          doneRef.current = false;
        })
        .then(() => scoreJob(posting.id));
    } else {
      // No persist needed — score immediately.
      doneRef.current = true;
      scoreJob(posting.id);
    }
  }, [
    description,
    descLoading,
    posting.id,
    resolved.isFetched,
    resolved.isFetching,
    resolvedLonger,
    scoreJob,
    shouldResolve,
    updateDescription,
  ]);

  // Polite AT announcement when the description upgrades to the full text.
  const [announced, setAnnounced] = useState(false);
  const prevDescLen = useRef(description.length);
  useEffect(() => {
    if (!announced && description.length > prevDescLen.current && prevDescLen.current > 0) {
      setAnnounced(true);
    }
    prevDescLen.current = description.length;
  }, [announced, description.length]);

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

  // Shared className for status Tag pills — applied/saved in the header.
  const statusTagCls = 'rounded-full px-1.5 py-0.5 text-[9px] uppercase tracking-wider';

  return (
    <motion.div
      {...variants.fadeSlideUp}
      transition={transition.fast}
      className="flex h-full flex-col overflow-hidden"
    >
      {/* Header — sticky: shrink-0 + overflow-y-auto on body achieves the sticky effect */}
      <div className="m-3 shrink-0 rounded-xl border border-[var(--border-clear)] p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          {/* LEFT: title + meta + match score + status tags */}
          <div className="min-w-0 flex-1">
            <h2 className="text-base font-semibold leading-snug text-foreground/95">
              {posting.title}
            </h2>
            {/* fold 10: bump metadata row from /60 to /70 (contrast floor at <14px) */}
            <div className="mt-1 flex flex-wrap items-center gap-2 text-[11px] text-foreground/70">
              <span className="font-semibold text-foreground/80">{posting.company}</span>
              {posting.location && (
                <span className="flex items-center gap-1">
                  <MapPin size={9} /> {posting.location}
                </span>
              )}
              {posting.remote && (
                <Tag
                  color="green"
                  className="rounded-full px-1.5 py-0.5 text-[9px] uppercase tracking-wider"
                >
                  {t('jobs.remote')}
                </Tag>
              )}
              <span role="presentation">
                <SourceBadge source={posting.source} url={posting.url} />
              </span>
              {posting.postedAt && <span>· {formatRelativeTime(posting.postedAt)}</span>}
            </div>
            <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
              <RowMatchScore jobId={posting.id} />
              {/* Status badges — viewed + applied + saved */}
              {(has('opened') || has('viewed')) && (
                <Tag color="blue" icon={<Eye size={8} />} className={statusTagCls}>
                  {t('jobs.viewed')}
                </Tag>
              )}
              {has('applied') && (
                <Tag color="purple" icon={<CircleCheck size={8} />} className={statusTagCls}>
                  {t('jobs.applied')}
                </Tag>
              )}
              {has('bookmarked') && (
                <Tag color="warning" icon={<Bookmark size={8} />} className={statusTagCls}>
                  {t('jobs.saved')}
                </Tag>
              )}
            </div>
          </div>

          {/* RIGHT: action cluster — Save/View, Tailor, ActionMenu */}
          <div className="flex shrink-0 flex-wrap items-center gap-2">
            <motion.div layout transition={transition.fast} className="shrink-0">
              <Button
                variant="primary"
                onClick={saved ? handleView : handleSave}
                disabled={pending}
                loading={pending}
                title={saved ? t('jobs.view') : t('applications.saveToTracking')}
              >
                {saved ? <Eye size={11} /> : <Save size={11} />}{' '}
                {saved ? t('jobs.view') : t('applications.save')}
              </Button>
            </motion.div>
            <Button
              variant="glass"
              onClick={() => void handleTailor()}
              title={t('jobs.tailorHint')}
            >
              <Wand2 size={11} /> {t('jobs.tailor')}
            </Button>
            <ActionMenu
              label={t('jobs.actions')}
              items={[
                { label: t('jobs.open'), icon: <ExternalLink size={14} />, onSelect: handleOpen },
                {
                  label: t('jobs.copyLink'),
                  icon: <Copy size={14} />,
                  onSelect: () => void handleCopyLink(),
                },
              ]}
            />
          </div>
        </div>
      </div>

      {/* Body — description.
          The live region is a small visually-hidden sentinel only (blocker 4):
          it announces the single "full description loaded" message once on
          the snippet→full upgrade, without noisily re-announcing the whole body. */}
      <div className="min-h-0 flex-1 overflow-y-auto pb-4 pl-5 pr-0 pt-1">
        {/* Visually-hidden AT sentinel — announces once when description upgrades */}
        <span role="status" aria-live="polite" aria-atomic="true" className="sr-only">
          {announced ? t('jobs.fullDescriptionLoaded') : ''}
        </span>

        {/* "About the job" section label */}
        <h3 className="mb-3 text-[11px] uppercase tracking-wider text-muted-foreground">
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
            aria-live="polite"
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
              className="max-w-prose space-y-4 text-caption text-foreground/80"
            />

            {/* blocker 7: show error hint when resolve failed AND the gate fired;
                gate on shouldResolve so non-aggregator postings are never affected */}
            {shouldResolve && resolved.isError && !descLoading && (
              <p className="mt-2 text-[11px] text-foreground/50">
                {t('jobs.descriptionLoadError')}
              </p>
            )}

            {/* Load button is OUTSIDE the live region (blocker 4) */}
            {showLoadButton && (
              <Button
                variant="ghost"
                onClick={() => void resolved.refetch()}
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
