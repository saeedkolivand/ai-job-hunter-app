import {
  Bookmark,
  Building2,
  CircleCheck,
  Copy,
  ExternalLink,
  Eye,
  MapPin,
  Wand2,
} from 'lucide-react';
import { useState } from 'react';
import { useNavigate } from '@tanstack/react-router';

import { useTranslation } from '@ajh/translations';
import { ActionMenu, Button, SourceBadge, useNotification } from '@ajh/ui';

import { RowMatchScore } from '@/features/jobs/components/RowMatchScore';
import { useOpenExternal, usePersistJob } from '@/services';
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

export function PostingRow({ posting, formatRelativeTime }: PostingRowProps) {
  const { t } = useTranslation();
  const notify = useNotification();
  const navigate = useNavigate();
  const setAIGenerate = useSessionStore((s) => s.setAIGenerate);
  const openExternalMutation = useOpenExternal();
  const persistJobMutation = usePersistJob();

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

  // Apply assistant: seed the AI Generate workspace with this posting, open the
  // posting so the user can submit it there, mark it applied, and route to the
  // tailoring flow. Tailoring is board-agnostic, so this works for every source.
  const handleTailor = () => {
    void trackInteraction('applied');
    setAIGenerate({ jobAd: posting.description, stage: 'idle', meta: null });
    void openExternalMutation.mutateAsync(posting.url);
    void navigate({ to: '/ai-generate' });
  };

  // The whole row opens the posting (#3). It's a role="button" region (not a
  // <button>) so the inline SourceBadge / action cluster can keep their own
  // click handlers — those stop propagation so they don't also open the row.
  const onRowKey = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      handleOpen();
    }
  };

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={handleOpen}
      onKeyDown={onRowKey}
      title={t('jobs.open')}
      className="surface-card group flex items-center gap-5 rounded-xl p-4 pl-5 transition-colors hover:bg-foreground/[0.03]"
    >
      <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-brand/10 text-[11px] font-semibold uppercase tracking-wider text-brand-soft">
        {posting.source.slice(0, 2)}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2 text-sm font-semibold tracking-tight text-foreground/95">
          <span className="truncate">{posting.title}</span>
          {posting.remote && (
            <span className="rounded-full border border-emerald-400/20 bg-emerald-400/5 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-emerald-200/85">
              {t('jobs.remote')}
            </span>
          )}
          {interactionTypes.has('applied') && (
            <span className="flex items-center gap-1 rounded-full border border-purple-400/20 bg-purple-400/5 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-purple-200/85">
              <CircleCheck size={8} /> {t('jobs.applied')}
            </span>
          )}
          {interactionTypes.has('opened') && (
            <span className="flex items-center gap-1 rounded-full border border-blue-400/20 bg-blue-400/5 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-blue-200/85">
              <Eye size={8} /> {t('jobs.viewed')}
            </span>
          )}
          {interactionTypes.has('bookmarked') && (
            <span className="flex items-center gap-1 rounded-full border border-amber-400/20 bg-amber-400/5 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-amber-200/85">
              <Bookmark size={8} /> {t('jobs.saved')}
            </span>
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
          {/* Badge keeps its own click (open source); stop it bubbling to the row. */}
          <span onClick={(e) => e.stopPropagation()}>
            <SourceBadge source={posting.source} url={posting.url} />
          </span>
          {posting.postedAt && (
            <span className="text-foreground/40">· {formatRelativeTime(posting.postedAt)}</span>
          )}
        </div>
      </div>
      {/* Action cluster — stops propagation so its buttons don't open the row. */}
      <div
        className="flex shrink-0 items-center gap-2"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
        role="presentation"
      >
        <RowMatchScore jobId={posting.id} />
        <Button
          size="sm"
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
