import { Loader2, Plus, Search } from 'lucide-react';
import { useMemo, useRef } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';

import { useTranslation } from '@ajh/translations';
import { Button, EmptyState, GlassCard, RowSkeleton } from '@ajh/ui';

import { PostingRow } from '@/features/jobs/components/PostingRow';
import { useMatchScores } from '@/features/jobs/providers';
import type { Posting } from '@/features/jobs/types';

interface JobsResultsProps {
  filtered: Posting[];
  formatRelativeTime: (timestamp?: number) => string;
  scraping: boolean;
  onShowMore: () => void;
  onScrape: () => void;
}

/**
 * Owns the results scroll area. Rendered INSIDE `MatchScoresProvider` so it can
 * read the batch-score context (the page itself provides that context and so
 * cannot consume it).
 *
 * Gating: results stay hidden behind a loading state while a scrape is running
 * OR (when a résumé exists) while the match-score batch is in flight. The list
 * is revealed once scraping has finished AND either every score is ready OR the
 * scoring query has errored out (error escape hatch — shows unscored results
 * rather than hanging on an infinite spinner).
 *
 * On reveal (résumé present), rows are re-sorted by `combined` score descending;
 * rows without a score sink to the bottom. `Array.sort` is stable, so ties keep
 * the incoming `filtered` order (already sorted by the user's `sortBy`). With no
 * résumé, `filtered` is shown as-is and only `scraping` gates.
 */
export function JobsResults({
  filtered,
  formatRelativeTime,
  scraping,
  onShowMore,
  onScrape,
}: JobsResultsProps) {
  const { t } = useTranslation();
  const { getScore, isPending, hasResume, isError } = useMatchScores();

  const waiting = scraping || (hasResume && isPending && !isError);

  // Re-sort by score on reveal. When the query data updates, the provider's
  // memoized `scoresById` Map changes identity, flowing a new `getScore` closure
  // into this memo's deps — so this re-runs once the batch settles, exactly when
  // the gate opens.
  const display = useMemo(
    () =>
      hasResume
        ? [...filtered].sort(
            (a, b) => (getScore(b.id)?.combined ?? -1) - (getScore(a.id)?.combined ?? -1)
          )
        : filtered,
    [filtered, hasResume, getScore]
  );

  // Windowed list: only visible rows (+ overscan) mount. Keyed by posting id so
  // measurement survives re-sorting and live-prepended rows during a scrape.
  const scrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: display.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 88,
    overscan: 6,
    getItemKey: (index) => display[index]?.id ?? index,
  });

  return (
    <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto px-10 pb-10">
      {waiting ? (
        <GlassCard>
          <div
            role="status"
            aria-live="polite"
            className="flex flex-col items-center justify-center gap-4 py-10"
          >
            <div className="flex items-center gap-2 text-sm text-foreground/70">
              <Loader2 size={16} className="animate-spin text-brand-soft" />
              {scraping ? t('jobs.searching') : t('jobs.scoring')}
            </div>
            <div className="w-full max-w-2xl space-y-2">
              <RowSkeleton />
              <RowSkeleton />
              <RowSkeleton />
            </div>
          </div>
        </GlassCard>
      ) : display.length === 0 ? (
        <GlassCard>
          <EmptyState
            icon={Search}
            title={t('jobs.empty')}
            action={
              <Button variant="primary" onClick={onScrape}>
                <Search size={13} /> {t('jobs.emptyCta')}
              </Button>
            }
            className="py-10"
          />
        </GlassCard>
      ) : (
        <div style={{ height: virtualizer.getTotalSize(), position: 'relative', width: '100%' }}>
          {virtualizer.getVirtualItems().map((vi) => {
            const posting = display[vi.index];
            if (!posting) return null;
            return (
              <div
                key={vi.key}
                data-index={vi.index}
                ref={virtualizer.measureElement}
                style={{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  width: '100%',
                  transform: `translateY(${vi.start}px)`,
                }}
              >
                {/* pb-2 reproduces the old gap-2 between rows (included in the
                  measured height so virtual offsets stay correct). */}
                <div className="pb-2">
                  <PostingRow posting={posting} formatRelativeTime={formatRelativeTime} />
                </div>
              </div>
            );
          })}
        </div>
      )}

      {!waiting && display.length > 0 && (
        <div className="flex justify-center pt-4">
          <Button variant="ghost" onClick={onShowMore} loading={scraping}>
            {!scraping && <Plus size={12} />}
            {t('jobs.showMore')}
          </Button>
        </div>
      )}
    </div>
  );
}
