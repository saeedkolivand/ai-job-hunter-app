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

import { useTranslation } from '@ajh/translations';
import { ActionMenu, Button, SourceBadge, Tag, transition } from '@ajh/ui';

import { CompanyAvatar } from '@/features/jobs/components/CompanyAvatar';
import { usePostingActions } from '@/features/jobs/hooks/usePostingActions';
import type { Posting } from '@/features/jobs/types';

interface PostingRowProps {
  posting: Posting;
  formatRelativeTime: (timestamp?: number) => string;
}

// Tiny status-pill shape for the in-row display Tags. Plain (non-CheckableTag)
// Tags render a <span> with no onClick, so clicks bubble to the row's handler
// instead of being swallowed — the whole row stays clickable.
const STATUS_TAG = 'rounded-full px-1.5 py-0.5 text-fine-print uppercase tracking-wider';

export function PostingRow({ posting, formatRelativeTime }: PostingRowProps) {
  const { t } = useTranslation();
  const { has, handleOpen, handleCopyLink, handleTailor, handleView, handleSave, saved, pending } =
    usePostingActions(posting);

  // The row is NOT clickable: a posting has no detail page, and clicking it must
  // not open the external job link. Opening the link stays available explicitly
  // via the row's "⋯ → Open" action. Save / Tailor are their own buttons.
  return (
    <div className="surface-card flex items-center gap-5 rounded-xl p-4 pl-5">
      <CompanyAvatar company={posting.company} sourceFallback={posting.source} size="md" />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2 text-caption-strong text-foreground/95">
          <span className="truncate">{posting.title}</span>
          {posting.remote && (
            <Tag color="green" className={STATUS_TAG}>
              {t('jobs.remote')}
            </Tag>
          )}
          {has('applied') && (
            <Tag color="purple" icon={<CircleCheck size={8} />} className={STATUS_TAG}>
              {t('jobs.applied')}
            </Tag>
          )}
          {(has('opened') || has('viewed')) && (
            <Tag color="blue" icon={<Eye size={8} />} className={STATUS_TAG}>
              {t('jobs.viewed')}
            </Tag>
          )}
          {has('bookmarked') && (
            <Tag color="warning" icon={<Bookmark size={8} />} className={STATUS_TAG}>
              {t('jobs.saved')}
            </Tag>
          )}
        </div>
        <div className="mt-1 flex items-center gap-4 text-fine-print">
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
        {/* motion wrapper animates the width change when Save flips to View. */}
        <motion.div layout transition={transition.fast} className="shrink-0">
          <Button
            variant="primary"
            onClick={saved ? handleView : handleSave}
            disabled={pending}
            title={saved ? t('jobs.view') : t('applications.saveToTracking')}
            loading={pending}
            className="transition-all duration-150 ease-out"
          >
            {saved ? <Eye size={11} /> : <Save size={11} />}{' '}
            {saved ? t('jobs.view') : t('applications.save')}
          </Button>
        </motion.div>
        <Button
          variant="glass"
          onClick={() => void handleTailor()}
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
