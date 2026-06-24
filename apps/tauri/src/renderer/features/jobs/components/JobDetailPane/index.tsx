import {
  Bookmark,
  Briefcase,
  CircleCheck,
  Copy,
  ExternalLink,
  Eye,
  Loader2,
  MapPin,
  Save,
  Wand2,
} from 'lucide-react';
import { motion } from 'motion/react';
import { useEffect, useRef } from 'react';

import { useTranslation } from '@ajh/translations';
import { ActionMenu, Button, EmptyState, SourceBadge, Tag, transition } from '@ajh/ui';

import { RowMatchScore } from '@/features/jobs/components/RowMatchScore';
import { usePostingActions } from '@/features/jobs/hooks/usePostingActions';
import type { Posting } from '@/features/jobs/types';
import { useResolveJobUrl } from '@/services';

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

  // Fall back to on-demand resolve when description is empty.
  const descriptionEmpty = !posting.description?.trim();
  const resolved = useResolveJobUrl(posting.url, descriptionEmpty);
  const description = descriptionEmpty ? (resolved.data?.description ?? '') : posting.description;
  const descLoading = descriptionEmpty && resolved.isLoading;

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
      <div className="shrink-0 border-b border-[var(--border-clear)] pl-5 pr-0 py-4">
        <div className="mb-1 flex items-start gap-3">
          <div className="min-w-0 flex-1">
            <h2 className="text-base font-semibold leading-snug text-foreground/95">
              {posting.title}
            </h2>
            <div className="mt-1 flex flex-wrap items-center gap-2 text-[11px] text-foreground/60">
              <span className="font-medium text-foreground/80">{posting.company}</span>
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

      {/* Body — description */}
      <div className="min-h-0 flex-1 overflow-y-auto pl-5 pr-0 py-4">
        {descLoading ? (
          <div className="flex items-center gap-2 text-sm text-foreground/50">
            <Loader2 size={14} className="animate-spin" />
            {t('jobs.loadingDescription')}
          </div>
        ) : (
          <p className="max-w-prose whitespace-pre-wrap text-[13px] leading-relaxed text-foreground/80">
            {description}
          </p>
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
