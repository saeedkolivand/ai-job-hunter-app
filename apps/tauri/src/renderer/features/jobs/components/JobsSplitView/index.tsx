import { ChevronLeft, Plus } from 'lucide-react';
import { useCallback, useEffect, useRef } from 'react';
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
  const { selectedId } = jobs;

  const selectedPosting = display.find((p) => p.id === selectedId) ?? null;

  const listScrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: display.length,
    getScrollElement: () => listScrollRef.current,
    estimateSize: () => 76,
    overscan: 6,
    getItemKey: (index) => display[index]?.id ?? index,
  });

  const handleSelect = (posting: Posting) => {
    setJobs({ selectedId: posting.id });
  };

  // Deferred focus-restore for the Back button on narrow screens:
  // The <aside> is still `hidden` at the moment handleBack fires (it becomes `flex`
  // only after the re-render when selectedPosting is null). Focusing synchronously
  // would drop focus on a hidden element. Instead we set a flag and an effect runs
  // AFTER the re-render — by which point the aside is visible and focusable.
  const restoreListFocusRef = useRef(false);
  const handleBack = () => {
    restoreListFocusRef.current = true;
    setJobs({ selectedId: null });
  };
  useEffect(() => {
    if (selectedId === null && restoreListFocusRef.current) {
      restoreListFocusRef.current = false;
      listScrollRef.current?.focus();
    }
  }, [selectedId]);

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
      setJobs({ selectedId: next.id });
      virtualizer.scrollToIndex(nextIndex, { align: 'auto' });
    },
    [display, selectedId, setJobs, virtualizer]
  );

  return (
    // flex-col on mobile, flex-row on md+. min-h-0 required for nested overflow.
    <div className="flex h-full min-h-0 flex-col md:flex-row">
      {/* ── Left: list pane ── */}
      {/* Narrow: hidden when a job is selected. Wide: fixed width, always visible. */}
      <aside
        className={[
          'flex flex-col overflow-hidden border-r border-[var(--border-clear)]',
          selectedPosting ? 'hidden md:flex' : 'flex',
          'md:w-[420px] xl:w-[480px] 2xl:w-[520px]',
        ].join(' ')}
      >
        {/* List scroll container — virtualizer targets this.
            tabIndex={0}: container is the sole tab stop (active-descendant pattern).
            handleBack also focuses it programmatically to return keyboard focus after
            closing detail on narrow screens. */}
        <div
          ref={listScrollRef}
          tabIndex={0}
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
      {/* Narrow: shown only when a job is selected. Wide: always visible. */}
      <section
        className={[
          'flex-col overflow-hidden min-w-0 flex-1',
          selectedPosting ? 'flex' : 'hidden md:flex',
        ].join(' ')}
      >
        {/* Controls bar — mobile Back button only; desktop never shows it */}
        {selectedPosting && (
          <div className="flex shrink-0 items-center gap-2 border-b border-[var(--border-clear)] py-2 pl-5 pr-0 md:hidden">
            <Button variant="ghost" onClick={handleBack} aria-label={t('jobs.backToList')}>
              <ChevronLeft size={13} /> {t('jobs.backToList')}
            </Button>
          </div>
        )}
        <div className="min-h-0 flex-1 overflow-hidden">
          <JobDetailPane posting={selectedPosting} formatRelativeTime={formatRelativeTime} />
        </div>
      </section>
    </div>
  );
}
