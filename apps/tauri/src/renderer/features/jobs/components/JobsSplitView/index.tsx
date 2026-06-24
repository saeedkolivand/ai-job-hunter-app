import { ChevronLeft, PanelRightClose, PanelRightOpen, Plus } from 'lucide-react';
import { useCallback, useRef } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';

import { useTranslation } from '@ajh/translations';
import { Button } from '@ajh/ui';

import { JobDetailPane } from '@/features/jobs/components/JobDetailPane';
import { PostingListItem } from '@/features/jobs/components/PostingListItem';
import type { Posting } from '@/features/jobs/types';
import { useSessionStore } from '@/store/session-store';

interface JobsSplitViewProps {
  display: Posting[];
  formatRelativeTime: (timestamp?: number) => string;
  scraping: boolean;
  onShowMore: () => void;
}

export function JobsSplitView({
  display,
  formatRelativeTime,
  scraping,
  onShowMore,
}: JobsSplitViewProps) {
  const { t } = useTranslation();
  const { jobs, setJobs } = useSessionStore();
  const { selectedId, detailCollapsed } = jobs;

  const selectedPosting = display.find((p) => p.id === selectedId) ?? null;

  // Detail is visible when a job is selected AND not collapsed.
  const showDetail = selectedPosting !== null && !detailCollapsed;

  const listScrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: display.length,
    getScrollElement: () => listScrollRef.current,
    estimateSize: () => 60,
    overscan: 6,
    getItemKey: (index) => display[index]?.id ?? index,
  });

  const handleSelect = (posting: Posting) => {
    setJobs({ selectedId: posting.id, detailCollapsed: false });
  };

  const handleBack = () => {
    setJobs({ detailCollapsed: true });
    // Return focus to the list scroll container so keyboard users can keep navigating.
    listScrollRef.current?.focus();
  };

  const handleCollapseDetail = () => setJobs({ detailCollapsed: true });
  const handleExpandDetail = () => setJobs({ detailCollapsed: false });

  // Arrow key navigation on the listbox: moves selection + scrolls to keep it visible.
  const handleListKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLDivElement>) => {
      if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
      e.preventDefault();
      const currentIndex = display.findIndex((p) => p.id === selectedId);
      const nextIndex =
        e.key === 'ArrowDown'
          ? Math.min(currentIndex + 1, display.length - 1)
          : Math.max(currentIndex - 1, 0);
      const next = display[nextIndex];
      if (!next) return;
      setJobs({ selectedId: next.id, detailCollapsed: false });
      virtualizer.scrollToIndex(nextIndex, { align: 'auto' });
    },
    [display, selectedId, setJobs, virtualizer]
  );

  return (
    // flex-col on mobile, flex-row on md+. min-h-0 required for nested overflow.
    <div className="flex h-full min-h-0 flex-col md:flex-row">
      {/* ── Left: list pane ── */}
      {/* Narrow: hidden when showing detail. Wide: fixed width, always visible
          unless detail is explicitly collapsed (then full width). */}
      <aside
        className={[
          'flex flex-col overflow-hidden border-r border-[var(--border-clear)]',
          showDetail ? 'hidden md:flex' : 'flex',
          // When collapsed+selected: aside fills available space but leaves room for the
          // expand strip (section keeps md:min-w-[40px]), so we use flex-1 rather than w-full.
          detailCollapsed && selectedPosting
            ? 'md:flex-1'
            : detailCollapsed
              ? 'md:w-full'
              : 'md:w-[320px] xl:w-[380px] 2xl:w-[420px]',
        ].join(' ')}
      >
        {/* List scroll container — virtualizer targets this.
            tabIndex={-1} so handleBack can programmatically focus it. */}
        <div
          ref={listScrollRef}
          tabIndex={-1}
          className="min-h-0 flex-1 overflow-y-auto focus-visible:outline-none"
          role="listbox"
          aria-label={t('jobs.title')}
          aria-activedescendant={selectedId ? `posting-${selectedId}` : undefined}
          onKeyDown={handleListKeyDown}
        >
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
                  <PostingListItem
                    posting={posting}
                    selected={posting.id === selectedId}
                    formatRelativeTime={formatRelativeTime}
                    onSelect={handleSelect}
                  />
                </div>
              );
            })}
          </div>
        </div>

        {/* Show more — mirrors list-mode footer; stays pinned below the scrollable list */}
        <div className="flex shrink-0 items-center justify-center border-t border-[var(--border-clear)] py-2">
          <Button variant="ghost" onClick={onShowMore} loading={scraping}>
            {!scraping && <Plus size={12} />}
            {t('jobs.showMore')}
          </Button>
        </div>
      </aside>

      {/* ── Right: detail pane ── */}
      {/* Narrow: shown only when detail is active. Wide: always visible.
          When collapsed+selected: shrinks to a 40px strip showing the expand button
          so it remains reachable by keyboard users. */}
      <section
        className={[
          'flex-col overflow-hidden',
          showDetail ? 'flex' : 'hidden md:flex',
          // Collapsed+selected: fixed strip so expand button stays visible.
          detailCollapsed && selectedPosting ? 'md:w-10 md:min-w-[40px]' : 'min-w-0 flex-1',
        ].join(' ')}
      >
        {/* Controls bar — only rendered when there is a selection (avoids phantom strip) */}
        {(selectedPosting || showDetail) && (
          <div className="flex shrink-0 items-center gap-2 border-b border-[var(--border-clear)] py-2 pl-5 pr-0">
            {/* Back button — narrow only */}
            <Button
              variant="ghost"
              onClick={handleBack}
              className="md:hidden"
              aria-label={t('jobs.backToList')}
            >
              <ChevronLeft size={13} /> {t('jobs.backToList')}
            </Button>
            {/* Collapse button — wide only, when detail is visible */}
            {selectedPosting && !detailCollapsed && (
              <Button
                variant="ghost"
                onClick={handleCollapseDetail}
                className="hidden md:flex"
                aria-label={t('jobs.collapseDetail')}
                title={t('jobs.collapseDetail')}
              >
                <PanelRightClose size={13} />
              </Button>
            )}
            {/* Expand button — wide only, when detail is collapsed but a job is selected */}
            {selectedPosting && detailCollapsed && (
              <Button
                variant="ghost"
                onClick={handleExpandDetail}
                className="hidden md:flex"
                aria-label={t('jobs.expandDetail')}
                title={t('jobs.expandDetail')}
              >
                <PanelRightOpen size={13} />
              </Button>
            )}
          </div>
        )}
        <div className="min-h-0 flex-1 overflow-hidden">
          <JobDetailPane posting={selectedPosting} formatRelativeTime={formatRelativeTime} />
        </div>
      </section>
    </div>
  );
}
