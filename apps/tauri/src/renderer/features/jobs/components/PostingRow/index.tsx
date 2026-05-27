import {
  Bookmark,
  Building2,
  CircleCheck,
  Copy,
  ExternalLink,
  Eye,
  MapPin,
  Send,
} from 'lucide-react';
import { motion } from 'motion/react';
import { useState } from 'react';

import { Button, SourceBadge, cn, transition, useNotification } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { useOpenExternal, usePersistJob } from '@/services';

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
  onApply: () => void;
  formatRelativeTime: (timestamp?: number) => string;
}

const APPLIABLE = new Set(['linkedin', 'indeed', 'greenhouse', 'workday', 'xing', 'glassdoor']);

export function PostingRow({ posting, onApply, formatRelativeTime }: PostingRowProps) {
  const { t } = useTranslation();
  const notify = useNotification();
  const canApply = APPLIABLE.has(posting.source);
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
      notify(t('jobs.copyLink'), 'success');
    } catch {
      notify('Failed to copy link', 'error');
    }
  };

  const handleApply = () => {
    void trackInteraction('applied');
    onApply();
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={transition.normal}
      className="relative group"
    >
      <div className="glass-graphite glass-highlight relative flex items-center gap-5 rounded-xl p-4 pl-5 transition-all duration-300 hover:bg-white/[0.03] hover:shadow-lg hover:shadow-brand/5 overflow-hidden">
        {/* Subtle ambient glow for whole card */}
        <div
          className="pointer-events-none absolute inset-0 opacity-0 group-hover:opacity-100 transition-opacity duration-300 ease-out"
          style={{
            background:
              'linear-gradient(135deg, rgba(168,85,247,0.12) 0%, rgba(99,102,241,0.08) 50%, rgba(168,85,247,0.12) 100%)',
            filter: 'blur-xl',
          }}
        />
        <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-white/10 to-white/5 text-[11px] uppercase tracking-wider text-brand-soft font-semibold shadow-inner">
          {posting.source.slice(0, 2)}
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 text-sm font-semibold text-foreground/95 tracking-tight">
            <span className="truncate">{posting.title}</span>
            {posting.remote && (
              <span className="rounded-full border border-emerald-400/20 bg-emerald-400/5 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-emerald-200/85">
                {t('jobs.remote')}
              </span>
            )}
            {/* Interaction indicators */}
            {interactionTypes.has('applied') && (
              <span className="rounded-full border border-purple-400/20 bg-purple-400/5 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-purple-200/85 flex items-center gap-1">
                <CircleCheck size={8} /> {t('jobs.applied')}
              </span>
            )}
            {interactionTypes.has('opened') && (
              <span className="rounded-full border border-blue-400/20 bg-blue-400/5 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-blue-200/85 flex items-center gap-1">
                <Eye size={8} /> {t('jobs.viewed')}
              </span>
            )}
            {interactionTypes.has('bookmarked') && (
              <span className="rounded-full border border-amber-400/20 bg-amber-400/5 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-amber-200/85 flex items-center gap-1">
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
            <SourceBadge source={posting.source} url={posting.url} />
            {posting.postedAt && (
              <span className="text-foreground/40">· {formatRelativeTime(posting.postedAt)}</span>
            )}
          </div>
        </div>
        <div className="flex items-center gap-2">
          <a
            href={posting.url}
            onClick={(e) => {
              e.preventDefault();
              void handleOpen();
            }}
            className="flex items-center gap-1.5 rounded-lg bg-white/5 px-2.5 py-1.5 text-[11px] text-foreground/70 hover:text-foreground hover:bg-white/10 transition-all duration-200"
          >
            <ExternalLink size={10} /> {t('jobs.open')}
          </a>
          <button
            onClick={handleCopyLink}
            className="flex items-center gap-1.5 rounded-lg bg-white/5 px-2.5 py-1.5 text-[11px] text-foreground/70 hover:text-foreground hover:bg-white/10 transition-all duration-200"
            title={t('jobs.copyLink')}
          >
            <Copy size={10} />
          </button>
          <Button
            size="sm"
            variant={canApply ? 'glass' : 'ghost'}
            onClick={handleApply}
            disabled={!canApply}
            title={canApply ? '' : t('jobs.applyNotSupported')}
            className={cn(
              'transition-all duration-150 ease-out',
              canApply ? '' : 'cursor-not-allowed'
            )}
          >
            <Send size={11} /> {t('jobs.apply')}
          </Button>
        </div>
      </div>
    </motion.div>
  );
}
