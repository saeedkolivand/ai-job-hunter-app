import { Bookmark, CircleCheck } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { cn } from '@ajh/ui';

import { useRowMatchScore } from '@/features/jobs/providers';
import type { Posting } from '@/features/jobs/types';
import { MatchBand } from '@/lib/match-band';

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
  const { score } = useRowMatchScore(posting.id);

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
        'flex h-[60px] cursor-pointer items-center gap-2 border-b border-[var(--border-clear)] px-2 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-brand',
        selected ? 'border-l-2 border-l-brand bg-brand/15' : 'hover:bg-muted'
      )}
    >
      {/* 32×32 source-badge logo slot — mirrors PostingRow's 40×40 at smaller size */}
      <div
        aria-hidden="true"
        className="flex h-8 w-8 shrink-0 items-center justify-center rounded bg-brand/10 text-[10px] font-semibold text-brand-soft"
      >
        {sourceBadge}
      </div>

      {/* Text block: 2-line layout fills remaining space */}
      <div className="min-w-0 flex-1">
        {/* Line 1: title + subtle MatchBand */}
        <div className="flex items-center gap-1.5">
          <span
            className={cn(
              'min-w-0 flex-1 truncate text-[12px] font-semibold',
              // Selected: full-opacity foreground.
              // Viewed (not selected): dimmed title per LinkedIn-style treatment.
              selected ? 'text-foreground' : isViewed ? 'text-foreground/55' : 'text-foreground/90'
            )}
          >
            {posting.title}
          </span>
          {score && <MatchBand value={score.combined} subtle />}
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
            'flex items-center gap-1.5 text-[10px]',
            // Viewed (not selected): dimmed meta.
            selected ? 'text-brand-soft/70' : isViewed ? 'text-foreground/40' : 'text-foreground/50'
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
              <span aria-hidden="true" className="text-[9px]">
                {t('jobs.viewed')}
              </span>
            )}
            {has('bookmarked') && <Bookmark size={9} aria-hidden="true" />}
          </span>
        </div>
      </div>
    </div>
  );
}
