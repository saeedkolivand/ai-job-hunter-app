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
import { ActionMenu, Button, EmptyState, SourceBadge, Tag, transition } from '@ajh/ui';

import { RowMatchScore } from '@/features/jobs/components/RowMatchScore';
import { usePostingActions } from '@/features/jobs/hooks/usePostingActions';
import {
  type DescriptionBlock,
  formatJobDescription,
} from '@/features/jobs/lib/format-description';
import type { Posting } from '@/features/jobs/types';
import { useInvalidateMatchBatch, useResolveJobUrl, useUpdatePostingDescription } from '@/services';

// ponytail: heuristic threshold — Adzuna search snippets are ~200–500 chars;
// anything under 700 chars for aggregator postings gets an on-demand resolve.
const SHORT_DESCRIPTION_CHARS = 700;

interface JobDetailPaneProps {
  posting: Posting | null;
  formatRelativeTime: (timestamp?: number) => string;
}

/** Exhaustive block renderer with compile-time guard for future variants. */
function DescriptionBlockView({ block, index }: { block: DescriptionBlock; index: number }) {
  switch (block.type) {
    case 'heading':
      return (
        <h3 key={index} className="mt-2 font-semibold leading-snug text-foreground/90 first:mt-0">
          {block.text}
        </h3>
      );
    case 'list':
      return (
        <ul key={index} className="list-disc space-y-1 pl-3 leading-relaxed">
          {block.items.map((item, j) => (
            <li key={j}>{item}</li>
          ))}
        </ul>
      );
    case 'paragraph':
      return (
        <p key={index} className="leading-relaxed">
          {block.text}
        </p>
      );
    default: {
      // Exhaustiveness guard — triggers a TypeScript error if a new
      // DescriptionBlock variant is added without updating this renderer.
      const _exhaustive: never = block;
      return null;
    }
  }
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

  // Re-score on open: when the resolved description is longer than the original
  // snippet, persist it back to the backend cache and invalidate the batch
  // match-score query so this job re-scores on the full text. Fires once per
  // posting (guarded by `upgraded` ref so it doesn't loop on re-renders).
  // Destructure mutateAsync so the dep array holds the stable function ref (RQ v5),
  // not the mutation object (new ref every render).
  const { mutateAsync: updateDescription } = useUpdatePostingDescription();
  const invalidateMatchBatch = useInvalidateMatchBatch();
  const upgraded = useRef(false);
  useEffect(() => {
    if (!upgraded.current && resolvedLonger && description) {
      upgraded.current = true;
      void updateDescription({ id: posting.id, description })
        .then(() => invalidateMatchBatch())
        .catch(() => {
          // Persist failure is non-fatal: the snippet score remains.
          // The one-shot guard stays true so we don't loop on rejected persists.
        });
    }
  }, [description, invalidateMatchBatch, posting.id, resolvedLonger, updateDescription]);

  // Polite AT announcement when the description upgrades to the full text.
  // We track the previous description length and announce once on the transition.
  const [announced, setAnnounced] = useState(false);
  const prevDescLen = useRef(description.length);
  useEffect(() => {
    if (!announced && description.length > prevDescLen.current && prevDescLen.current > 0) {
      setAnnounced(true);
    }
    prevDescLen.current = description.length;
  }, [announced, description.length]);

  // Mark 'viewed' once on display (effect keyed by posting.id; dedupe via Rust upsert).
  // Use a stable ref so the effect dep is only posting.id (the identity sentinel).
  const trackInteractionRef = useRef(trackInteraction);
  trackInteractionRef.current = trackInteraction;
  useEffect(() => {
    void trackInteractionRef.current('viewed');
  }, [posting.id]);

  return (
    <div className="flex h-full flex-col overflow-hidden">
      {/* Header */}
      <div className="shrink-0 border-b border-[var(--border-clear)] py-4 pl-5 pr-0">
        <div className="mb-1 flex items-start gap-3">
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
              <span role="presentation">
                <SourceBadge source={posting.source} url={posting.url} />
              </span>
              {posting.postedAt && <span>· {formatRelativeTime(posting.postedAt)}</span>}
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-1.5">
            <RowMatchScore jobId={posting.id} />
          </div>
        </div>

        {/* Action cluster */}
        <div className="mt-3 flex items-center gap-2">
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
          <Button variant="glass" onClick={() => void handleTailor()} title={t('jobs.tailorHint')}>
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
          {/* Status badges — use Tag primitives, mirror PostingRow */}
          {has('applied') && (
            <Tag
              color="purple"
              icon={<CircleCheck size={8} />}
              className="rounded-full px-1.5 py-0.5 text-[9px] uppercase tracking-wider"
            >
              {t('jobs.applied')}
            </Tag>
          )}
          {(has('opened') || has('viewed')) && (
            <Tag
              color="blue"
              icon={<Eye size={8} />}
              className="rounded-full px-1.5 py-0.5 text-[9px] uppercase tracking-wider"
            >
              {t('jobs.viewed')}
            </Tag>
          )}
          {has('bookmarked') && (
            <Tag
              color="warning"
              icon={<Bookmark size={8} />}
              className="rounded-full px-1.5 py-0.5 text-[9px] uppercase tracking-wider"
            >
              {t('jobs.saved')}
            </Tag>
          )}
        </div>
      </div>

      {/* Body — description.
          The live region is a small visually-hidden sentinel only (blocker 4):
          it announces the single "full description loaded" message once on
          the snippet→full upgrade, without noisily re-announcing the whole body. */}
      <div className="min-h-0 flex-1 overflow-y-auto py-4 pl-5 pr-0">
        {/* Visually-hidden AT sentinel — announces once when description upgrades */}
        <span role="status" aria-live="polite" aria-atomic="true" className="sr-only">
          {announced ? t('jobs.fullDescriptionLoaded') : ''}
        </span>

        {descLoading ? (
          <div
            role="status"
            aria-busy="true"
            className="flex items-center gap-2 text-sm text-foreground/70"
          >
            <Loader2 size={14} aria-hidden="true" className="animate-spin" />
            {t('jobs.loadingDescription')}
          </div>
        ) : (
          <>
            {/* fold 9: space-y-4 for block rhythm; headings use mt-2 not mt-4 */}
            <div className="max-w-prose space-y-4 text-[13px] text-foreground/80">
              {formatJobDescription(description).map((block, i) => (
                <DescriptionBlockView key={i} block={block} index={i} />
              ))}
            </div>

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
    </div>
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
