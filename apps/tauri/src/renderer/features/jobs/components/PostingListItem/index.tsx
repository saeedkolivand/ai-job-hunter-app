import { Bookmark, CircleCheck, Eye } from 'lucide-react';

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

  const handleClick = () => onSelect(posting);
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      onSelect(posting);
    }
  };

  return (
    <div
      id={`posting-${posting.id}`}
      role="option"
      aria-selected={selected}
      // Roving tabindex: only the selected item is tab-reachable; others via arrows.
      tabIndex={selected ? 0 : -1}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
      className={cn(
        'flex h-[60px] cursor-pointer flex-col justify-center gap-1 border-b border-[var(--border-clear)] py-2 pl-2 pr-4 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-brand',
        // Active: stronger brand fill + full-opacity text — mirrors sidebar NavPill weight.
        // Inactive: default muted hover.
        selected ? 'border-l-2 border-l-brand bg-brand/15' : 'hover:bg-muted'
      )}
    >
      {/* Line 1: title + subtle MatchBand */}
      <div className="flex items-center gap-1.5">
        <span
          className={cn(
            'min-w-0 flex-1 truncate text-[12px] font-semibold',
            // Selected: full-opacity foreground (mirrors sidebar active text-foreground).
            selected ? 'text-foreground' : 'text-foreground/90'
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

      {/* Line 2: company · location · time, then icon-only status markers */}
      {/* Selected: brand-soft tint on meta (mirrors sidebar icon text-brand-soft). */}
      <div
        className={cn(
          'flex items-center gap-1.5 text-[10px]',
          selected ? 'text-brand-soft/70' : 'text-foreground/50'
        )}
      >
        <span className="truncate">{posting.company}</span>
        {posting.location && <span className="shrink-0 truncate">· {posting.location}</span>}
        {posting.postedAt && (
          <span className="shrink-0">· {formatRelativeTime(posting.postedAt)}</span>
        )}
        {/* Icon-only interaction markers — decorative (aria-hidden); SR summary above */}
        <span className="ml-auto flex shrink-0 items-center gap-1">
          {has('applied') && <CircleCheck size={9} aria-hidden="true" />}
          {(has('opened') || has('viewed')) && <Eye size={9} aria-hidden="true" />}
          {has('bookmarked') && <Bookmark size={9} aria-hidden="true" />}
        </span>
      </div>
    </div>
  );
}
