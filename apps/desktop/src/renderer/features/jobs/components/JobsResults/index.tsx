import { Loader2, Plus, Search, Settings } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { useRouter } from '@tanstack/react-router';
import { useVirtualizer } from '@tanstack/react-virtual';

import { type BoardScrapeSummary, PROVIDER_SLOTS } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, EmptyState, GlassCard, ProgressBar, RowSkeleton } from '@ajh/ui';

import { BoardSummaryChips } from '@/components/scrape/BoardSummaryChips';
import { ROUTES } from '@/constants/routes/routes';
import { JobsSplitView } from '@/features/jobs/components/JobsSplitView';
import { PostingRow } from '@/features/jobs/components/PostingRow';
import type { Posting } from '@/features/jobs/types';
import { useHasProviderKey } from '@/services/use-ai-provider';
import { useSessionStore } from '@/store/session-store';

interface JobsResultsProps {
  filtered: Posting[];
  formatRelativeTime: (timestamp?: number) => string;
  scraping: boolean;
  /** Live boards-done/total fraction (0..1) for the active scrape; null until the
   *  first board completes and after the scrape ends. */
  scrapeProgress?: number | null;
  /** Per-board outcome of the most recent scrape — rendered in the empty state so
   *  a zero result always explains *why* per board (skipped / errored / partial). */
  boardSummaries?: BoardScrapeSummary[];
  /** Sanitized note for an outright (non-per-board) scrape failure — mutually
   *  exclusive in practice with `boardSummaries` (a `job.failed` clears it). */
  failureNote?: string | null;
  onShowMore: () => void;
  onScrape: () => void;
}

/**
 * Owns the results scroll area. Rendered INSIDE `MatchScoresProvider` so it can
 * read the score context (the page itself provides that context and so cannot
 * consume it).
 *
 * Gating: results stay hidden behind a loading state while a scrape is running.
 * Scores are fetched on-demand when the user opens a job (`JobDetailPane` calls
 * `scoreJob`) — no batch-all on mount, no score-based reorder. The list keeps
 * the user's chosen order (newest/oldest/company) at all times.
 */
export function JobsResults({
  filtered,
  formatRelativeTime,
  scraping,
  scrapeProgress,
  boardSummaries,
  failureNote,
  onShowMore,
  onScrape,
}: JobsResultsProps) {
  const { t } = useTranslation();
  const router = useRouter();
  const jobs = useSessionStore((s) => s.jobs);
  const setJobs = useSessionStore((s) => s.setJobs);
  const { viewMode, selectedId } = jobs;
  const setSettings = useSessionStore((s) => s.setSettings);

  // Check whether Adzuna keys are configured — only query when the list is empty
  // and not scraping, to avoid unnecessary IPC calls during normal operation.
  const isEmpty = !scraping && filtered.length === 0;
  const { data: adzunaIdData, isSuccess: adzunaIdReady } = useHasProviderKey(
    PROVIDER_SLOTS.adzunaAppId,
    isEmpty
  );
  const { data: adzunaKeyData, isSuccess: adzunaKeyReady } = useHasProviderKey(
    PROVIDER_SLOTS.adzunaAppKey,
    isEmpty
  );
  const keysKnown = adzunaIdReady && adzunaKeyReady;
  const missingAdzunaKeys =
    isEmpty && keysKnown && (adzunaIdData?.has === false || adzunaKeyData?.has === false);

  // Show the full skeleton only on a fresh search (no results yet).
  // During show-more (scraping=true but results already visible) we keep the
  // list rendered — the "Show more" button's own loading={scraping} covers the
  // in-progress state without displacing existing results.
  const waiting = scraping && filtered.length === 0;

  // Derive selection validity during render so display changes flow into deps.
  const topId = filtered[0]?.id ?? null;
  const selectionInDisplay = selectedId !== null && filtered.some((p) => p.id === selectedId);

  // Auto-select: pick topId only when there is NO valid selection in the display.
  // This preserves the user's chosen job across re-scrapes, show-more, and live
  // prepends — we only step in when the detail pane would otherwise be blank
  // (fresh search with nothing selected, or selection was filtered out).
  useEffect(() => {
    if (viewMode !== 'split' || topId === null) return;
    if (!selectionInDisplay) {
      setJobs({ selectedId: topId });
    }
  }, [viewMode, topId, selectionInDisplay, setJobs]);

  // Windowed list: only visible rows (+ overscan) mount. Keyed by posting id so
  // measurement survives live-prepended rows during a scrape.
  const scrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: filtered.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 88,
    overscan: 6,
    getItemKey: (index) => filtered[index]?.id ?? index,
  });

  const openAggregatorSettings = () => {
    setSettings({ activeSection: 'job' });
    void router.navigate({ to: ROUTES.SETTINGS });
  };

  // Shared gating block — only scraping gates the list now.
  if (waiting) {
    return (
      <div className="min-h-0 flex-1 overflow-y-auto px-10 pb-10">
        <GlassCard>
          <div
            role="status"
            aria-live="polite"
            className="flex flex-col items-center justify-center gap-4 py-10"
          >
            <div className="flex items-center gap-2 text-sm text-foreground/70">
              <Loader2 size={16} className="animate-spin text-brand-soft" />
              {t('jobs.searching')}
            </div>
            <div className="w-full max-w-2xl space-y-2">
              {/* Transient live scrape progress — a thin fill that advances as
                  each board finishes; shown only while a fresh scrape has no
                  results yet. */}
              <div className="space-y-1">
                <ProgressBar value={(scrapeProgress ?? 0) * 100} showLabel={false} />
                <div className="text-[10px] text-foreground/45">
                  {scrapeProgress == null
                    ? t('jobs.scanning')
                    : t('jobs.scanningPercent', { percent: Math.round(scrapeProgress * 100) })}
                </div>
              </div>
              <RowSkeleton />
              <RowSkeleton />
              <RowSkeleton />
            </div>
          </div>
        </GlassCard>
      </div>
    );
  }

  if (filtered.length === 0) {
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
                  <Button variant="primary" onClick={openAggregatorSettings}>
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
                -explaining the same root cause. */}
            {!missingAdzunaKeys && boardSummaries && boardSummaries.length > 0 && (
              <div className="flex justify-center px-6 pb-8">
                <BoardSummaryChips summaries={boardSummaries} />
              </div>
            )}
            {!missingAdzunaKeys && failureNote && (
              <p className="px-6 pb-8 text-center text-[11px] text-red-400/80">
                {t('jobs.lastScrapeFailed', { reason: failureNote })}
              </p>
            )}
          </div>
        </GlassCard>
      </div>
    );
  }

  // Split mode: two-pane layout — JobsSplitView owns the virtualizer + layout.
  // px-10 pb-10 aligns the list's left edge and the detail's right edge to the
  // page header's 40px horizontal margin.
  // surface-card wraps the two panes as one cohesive themed card (white in light
  // theme, dark tile in dark theme) so the split reads like a single LinkedIn-style card.
  if (viewMode === 'split') {
    return (
      <div className="min-h-0 flex-1 overflow-hidden px-10 pb-10">
        <div className="surface-card flex h-full min-h-0 overflow-hidden rounded-2xl">
          <JobsSplitView
            display={filtered}
            formatRelativeTime={formatRelativeTime}
            scraping={scraping}
            onShowMore={onShowMore}
          />
        </div>
      </div>
    );
  }

  // List mode: existing single-column virtualised rows
  return (
    <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto px-10 pb-10">
      <div style={{ height: virtualizer.getTotalSize(), position: 'relative', width: '100%' }}>
        {virtualizer.getVirtualItems().map((vi) => {
          const posting = filtered[vi.index];
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

      <div className="flex justify-center pt-4">
        <Button variant="primary" onClick={onShowMore} loading={scraping}>
          {!scraping && <Plus size={12} />}
          {t('jobs.showMore')}
        </Button>
      </div>
    </div>
  );
}
