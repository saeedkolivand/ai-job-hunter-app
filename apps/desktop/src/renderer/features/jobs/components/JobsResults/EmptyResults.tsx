import { Search, Settings } from 'lucide-react';

import type { BoardScrapeSummary } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, EmptyState, GlassCard } from '@ajh/ui';

import { BoardSummaryChips } from '@/components/scrape/BoardSummaryChips';

interface EmptyResultsProps {
  missingAdzunaKeys: boolean;
  /** "Genuinely zero postings" vs "a text filter hid everything". */
  genuinelyEmpty: boolean;
  boardSummaries?: BoardScrapeSummary[];
  failureNote?: string | null;
  onScrape: () => void;
  onOpenAggregatorSettings: () => void;
}

/** Zero-result state with the per-board diagnostics that explain it. */
export function EmptyResults({
  missingAdzunaKeys,
  genuinelyEmpty,
  boardSummaries,
  failureNote,
  onScrape,
  onOpenAggregatorSettings,
}: EmptyResultsProps) {
  const { t } = useTranslation();
  return (
    <div className="min-h-0 flex-1 overflow-y-auto px-10 pb-10">
      <GlassCard>
        <div role="status" aria-live="polite">
          {missingAdzunaKeys ? (
            <EmptyState
              icon={Search}
              title={t('jobs.empty')}
              description={t('jobs.emptyNoAdzunaKeys')}
              action={
                <Button variant="primary" onClick={onOpenAggregatorSettings}>
                  <Settings size={13} /> {t('jobs.emptyNoAdzunaKeysCta')}
                </Button>
              }
              className="py-10"
            />
          ) : (
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
          )}
          {/* Per-board diagnostics so a zero result is never silent — the same
              strip shown in the results header, wired here so the empty state
              explains which boards were skipped / errored / returned partial.
              Suppressed when `missingAdzunaKeys` already explains the zero
              (that branch renders its own dedicated CTA) to avoid triple
              -explaining the same root cause, AND when a text filter (not
              the scrape) is what emptied the list — `genuinelyEmpty` keeps a
              filter-hides-all view from re-showing a PRIOR scrape's outcome
              as if this scrape found nothing. */}
          {!missingAdzunaKeys && genuinelyEmpty && boardSummaries && boardSummaries.length > 0 && (
            <div className="flex justify-center px-6 pb-8">
              <BoardSummaryChips summaries={boardSummaries} />
            </div>
          )}
          {!missingAdzunaKeys && genuinelyEmpty && failureNote && (
            <p className="px-6 pb-8 text-center text-[11px] text-red-400/80">
              {t('jobs.lastScrapeFailed', { reason: failureNote })}
            </p>
          )}
        </div>
      </GlassCard>
    </div>
  );
}
