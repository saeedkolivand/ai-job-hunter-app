import { Bookmark, CircleCheck } from 'lucide-react';
import { motion } from 'motion/react';

import { useTranslation } from '@ajh/translations';
import { cn, transition } from '@ajh/ui';

import type { Posting } from '@/features/jobs/types';

interface PostingListItemProps {
  posting: Posting;
  selected: boolean;
  formatRelativeTime: (timestamp?: number) => string;
  onSelect: (posting: Posting) => void;
}

export function PostingListItem({
  posting,
  selected,
  formatRelativeTime,
  onSelect,
}: PostingListItemProps) {
  const { t } = useTranslation();

  const interactions = new Set<string>(posting.interactions?.map((i) => i.interactionType) ?? []);
  const has = (type: string) => interactions.has(type);

  const isViewed = has('opened') || has('viewed');

  const handleClick = () => onSelect(posting);
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      onSelect(posting);
    }
  };

  // Source badge: 2-letter abbreviation, 32×32, mirrors PostingRow's 40×40 at smaller size.
  const sourceBadge = posting.source.slice(0, 2).toUpperCase();

  return (
    <div
      id={`posting-${posting.id}`}
      role="option"
      aria-selected={selected}
      // Active-descendant pattern: focus stays on the listbox container.
      // Items are never tab stops; keyboard nav is handled by the container.
      tabIndex={-1}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
      className={cn(
        // Outer shell: fixed height for the virtualizer; relative for the abs-positioned
        // slide indicator. border-b separator on unselected rows only.
        'relative flex h-[76px] cursor-pointer items-center transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-brand',
        selected ? '' : 'border-b border-[var(--border-clear)] hover:bg-muted'
      )}
    >
      {/* Animated active highlight — slides between rows via shared-layout (mirrors sidebar).
          Square (rounded-none) + full-width; no inset; same brand fill as NavPill. */}
      {selected && (
        <motion.div
          aria-hidden
          layoutId="jobs-list-pill"
          className="pointer-events-none absolute inset-0 rounded-none"
          style={{
            background:
              'linear-gradient(135deg, color-mix(in srgb, var(--color-brand) 18%, transparent) 0%, color-mix(in srgb, var(--color-brand-2) 10%, transparent) 100%)',
            boxShadow: 'inset 0 0 16px color-mix(in srgb, var(--color-brand) 6%, transparent)',
          }}
          transition={transition.spring}
        />
      )}

      {/* Content layer — sits above the pill (pill is absolute inset-0) */}
      <div className="relative flex h-[64px] w-full items-center gap-3 px-2">
        {/* 32×32 source-badge logo slot — mirrors PostingRow's 40×40 at smaller size */}
        <div
          aria-hidden="true"
          className="flex h-8 w-8 shrink-0 items-center justify-center rounded bg-brand/10 text-[10px] font-semibold text-brand-soft"
        >
          {sourceBadge}
        </div>

        {/* Text block: 2-line layout fills remaining space */}
        <div className="min-w-0 flex-1">
          {/* Line 1: title only — score moved to detail pane */}
          <div className="flex items-center gap-1.5">
            <span
              className={cn(
                'min-w-0 flex-1 truncate text-[13px] font-semibold',
                // Selected: full-opacity foreground.
                // Viewed (not selected): dimmed title per LinkedIn-style treatment.
                selected
                  ? 'text-foreground'
                  : isViewed
                    ? 'text-muted-foreground'
                    : 'text-foreground/90'
              )}
            >
              {posting.title}
            </span>
          </div>

          {/* Visually-hidden status summary for screen readers */}
          {(has('applied') || has('opened') || has('viewed') || has('bookmarked')) && (
            <span className="sr-only">
              {[
                has('applied') && t('jobs.applied'),
                (has('opened') || has('viewed')) && t('jobs.viewed'),
                has('bookmarked') && t('jobs.saved'),
              ]
                .filter(Boolean)
                .join(', ')}
            </span>
          )}

          {/* Line 2: company · location · time, then status markers */}
          <div
            className={cn(
              'flex items-center gap-1.5 text-[11px]',
              selected ? 'text-brand-soft/70' : 'text-foreground/50'
            )}
          >
            <span className="truncate">{posting.company}</span>
            {posting.location && <span className="shrink-0 truncate">· {posting.location}</span>}
            {posting.postedAt && (
              <span className="shrink-0">· {formatRelativeTime(posting.postedAt)}</span>
            )}
            {/* Status markers — decorative (aria-hidden); SR summary above */}
            <span className="ml-auto flex shrink-0 items-center gap-1">
              {has('applied') && <CircleCheck size={9} aria-hidden="true" />}
              {/* "Viewed" text label replaces the eye icon — aria-hidden since SR uses the summary above */}
              {isViewed && !selected && (
                <span aria-hidden="true" className="text-[10px]">
                  {t('jobs.viewed')}
                </span>
              )}
              {has('bookmarked') && <Bookmark size={9} aria-hidden="true" />}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
