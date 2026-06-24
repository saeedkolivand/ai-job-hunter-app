import { Loader2, Plus, Search, Settings } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { useRouter } from '@tanstack/react-router';
import { useVirtualizer } from '@tanstack/react-virtual';

import { PROVIDER_SLOTS } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, EmptyState, GlassCard, RowSkeleton } from '@ajh/ui';

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
  onShowMore,
  onScrape,
}: JobsResultsProps) {
  const { t } = useTranslation();
  const router = useRouter();
  const { jobs, setJobs } = useSessionStore();
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

  // List is shown immediately once scraping finishes; scores arrive per-job on open.
  const waiting = scraping;

  // Derive selection validity during render so display changes flow into deps.
  const topId = filtered[0]?.id ?? null;
  const selectionInDisplay = selectedId !== null && filtered.some((p) => p.id === selectedId);

  // Unified selection effect — subsumes both the old null-reconciliation effect
  // and the split-mode auto-select:
  //
  //   In split mode with results:
  //     • justFinished (waiting true→false): re-select topId so fresh scrape
  //       results are immediately visible in the detail pane.
  //     • !selectionInDisplay: selection is absent OR was filtered out of the
  //       list; select topId so the detail pane is never left blank.
  //     • selectionInDisplay && !justFinished: user has a valid manual selection,
  //       leave it alone even if topId changes (live prepend, filter change).
  //
  //   In list mode: no-op — list mode has no auto-select requirement.
  //
  // Deps include derived booleans (topId, selectionInDisplay) so a filter change
  // triggers the effect even when selectedId hasn't changed.
  const prevWaitingRef = useRef(waiting);
  useEffect(() => {
    const justFinished = prevWaitingRef.current && !waiting;
    prevWaitingRef.current = waiting;

    if (viewMode !== 'split' || topId === null) return;

    if (justFinished || !selectionInDisplay) {
      setJobs({ selectedId: topId, detailCollapsed: false });
    }
  }, [waiting, viewMode, topId, selectionInDisplay, setJobs]);

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
          </div>
        </GlassCard>
      </div>
    );
  }

  // Split mode: two-pane layout — JobsSplitView owns the virtualizer + layout.
  // px-10 pb-10 aligns the list's left edge and the detail's right edge to the
  // page header's 40px horizontal margin.
  if (viewMode === 'split') {
    return (
      <div className="min-h-0 flex-1 overflow-hidden px-10 pb-10">
        <JobsSplitView
          display={filtered}
          formatRelativeTime={formatRelativeTime}
          scraping={scraping}
          onShowMore={onShowMore}
        />
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
        <Button variant="ghost" onClick={onShowMore} loading={scraping}>
          {!scraping && <Plus size={12} />}
          {t('jobs.showMore')}
        </Button>
      </div>
    </div>
  );
}
