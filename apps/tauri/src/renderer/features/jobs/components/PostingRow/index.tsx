import {
  Bookmark,
  Building2,
  CircleCheck,
  Copy,
  ExternalLink,
  Eye,
  MapPin,
  Save,
  Wand2,
} from 'lucide-react';
import { motion } from 'motion/react';
import { useState } from 'react';
import { useNavigate } from '@tanstack/react-router';

import { useTranslation } from '@ajh/translations';
import { ActionMenu, Button, SourceBadge, Tag, transition, useNotification } from '@ajh/ui';

import { RowMatchScore } from '@/features/jobs/components/RowMatchScore';
import { useOpenExternal, usePersistJob } from '@/services';
import { useSaveFromPosting } from '@/services/use-applications';
import { useSessionStore } from '@/store/session-store';

interface Posting {
  id: string;
  source: string;
  externalId: string;
  url: string;
  title: string;
  company: string;
  location?: string;
  remote?: boolean;
  description: string;
  postedAt?: number;
  capturedAt: number;
  interactions?: { interactionType: string }[];
}
interface PostingRowProps {
  posting: Posting;
  formatRelativeTime: (timestamp?: number) => string;
}

// Tiny status-pill shape for the in-row display Tags. Plain (non-CheckableTag)
// Tags render a <span> with no onClick, so clicks bubble to the row's handler
// instead of being swallowed — the whole row stays clickable.
const STATUS_TAG = 'rounded-full px-1.5 py-0.5 text-[9px] uppercase tracking-wider';

export function PostingRow({ posting, formatRelativeTime }: PostingRowProps) {
  const { t } = useTranslation();
  const notify = useNotification();
  const navigate = useNavigate();
  const setAIGenerate = useSessionStore((s) => s.setAIGenerate);
  const openExternalMutation = useOpenExternal();
  const persistJobMutation = usePersistJob();
  const saveFromPostingMutation = useSaveFromPosting();

  const [interactionTypes, setInteractionTypes] = useState(
    () => new Set(posting.interactions?.map((i) => i.interactionType) || [])
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

  const handleOpen = () => {
    void trackInteraction('opened');
    void openExternalMutation.mutateAsync(posting.url);
  };

  const handleCopyLink = async () => {
    try {
      await navigator.clipboard.writeText(posting.url);
      notify.success({ message: t('jobs.copyLink') });
    } catch {
      notify.error({ message: 'Failed to copy link' });
    }
  };

  const handleTailor = () => {
    void trackInteraction('applied');
    setAIGenerate({ jobAd: posting.description, stage: 'idle', meta: null });
    void openExternalMutation.mutateAsync(posting.url);
    void navigate({ to: '/ai-generate' });
  };

  // Once saved, the button becomes a "View" link to the tracking list.
  const handleView = () => void navigate({ to: '/applications' });

  // Jobs-page Save: create an Application with status=saved linked to this posting.
  const handleSave = () => {
    // Also mark the posting bookmarked — this persists, shows the "saved" badge,
    // and flips the Save button to the "View" link (optimistically + on reload).
    void trackInteraction('bookmarked');
    void saveFromPostingMutation.mutateAsync({
      jobUrl: posting.url,
      board: posting.source,
      company: posting.company,
      title: posting.title,
    });
    notify.success({ message: t('applications.savedToTracking') });
  };

  const saved = interactionTypes.has('bookmarked');

  // The row is NOT clickable: a posting has no detail page, and clicking it must
  // not open the external job link. Opening the link stays available explicitly
  // via the row's "⋯ → Open" action. Save / Tailor are their own buttons.
  return (
    <div className="surface-card flex items-center gap-5 rounded-xl p-4 pl-5">
      <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-brand/10 text-[11px] font-semibold uppercase tracking-wider text-brand-soft">
        {posting.source.slice(0, 2)}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2 text-sm font-semibold tracking-tight text-foreground/95">
          <span className="truncate">{posting.title}</span>
          {posting.remote && (
            <Tag color="green" className={STATUS_TAG}>
              {t('jobs.remote')}
            </Tag>
          )}
          {interactionTypes.has('applied') && (
            <Tag color="purple" icon={<CircleCheck size={8} />} className={STATUS_TAG}>
              {t('jobs.applied')}
            </Tag>
          )}
          {interactionTypes.has('opened') && (
            <Tag color="blue" icon={<Eye size={8} />} className={STATUS_TAG}>
              {t('jobs.viewed')}
            </Tag>
          )}
          {interactionTypes.has('bookmarked') && (
            <Tag color="warning" icon={<Bookmark size={8} />} className={STATUS_TAG}>
              {t('jobs.saved')}
            </Tag>
          )}
        </div>
        <div className="mt-1 flex items-center gap-4 text-[11px]">
          <span className="flex items-center gap-1.5 text-foreground/85">
            <Building2 size={10} /> {posting.company}
          </span>
          {posting.location && (
            <span className="flex items-center gap-1.5 text-foreground/60">
              <MapPin size={10} /> {posting.location}
            </span>
          )}
          <span
            role="presentation"
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => e.stopPropagation()}
          >
            <SourceBadge source={posting.source} url={posting.url} />
          </span>
          {posting.postedAt && (
            <span className="text-foreground/40">· {formatRelativeTime(posting.postedAt)}</span>
          )}
        </div>
      </div>
      <div
        className="flex shrink-0 items-center gap-2"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
        role="presentation"
      >
        <RowMatchScore jobId={posting.id} />
        {/* motion wrapper animates the width change when Save flips to View. */}
        <motion.div layout transition={transition.fast} className="shrink-0">
          <Button
            variant="primary"
            onClick={saved ? handleView : handleSave}
            disabled={saveFromPostingMutation.isPending}
            title={saved ? t('jobs.view') : t('applications.saveToTracking')}
            loading={saveFromPostingMutation.isPending}
            className="transition-all duration-150 ease-out"
          >
            {saved ? <Eye size={11} /> : <Save size={11} />}{' '}
            {saved ? t('jobs.view') : t('applications.save')}
          </Button>
        </motion.div>
        <Button
          variant="glass"
          onClick={handleTailor}
          title={t('jobs.tailorHint')}
          className="transition-all duration-150 ease-out"
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
  );
}
