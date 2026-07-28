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
  // Captured once at mount so the restore effect has a stable reference without
  // being reactive to subsequent scrollTop saves.
  const initialScrollTop = useRef(jobs.listScrollTop);
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

  // Restore scroll position when remounting after back-navigation (e.g. returning
  // from /applications/$id). Reads the value captured at mount — not reactive.
  useEffect(() => {
    const el = listScrollRef.current;
    if (el && initialScrollTop.current > 0) {
      el.scrollTop = initialScrollTop.current;
    }
  }, []); // intentional mount-only restore — body only reads stable refs

  // Persist scroll position into the session store (throttled) so the restore
  // above has a fresh value on the next mount.
  useEffect(() => {
    const el = listScrollRef.current;
    if (!el) return;
    let rafId: ReturnType<typeof requestAnimationFrame> | null = null;
    const onScroll = () => {
      if (rafId !== null) return;
      rafId = requestAnimationFrame(() => {
        rafId = null;
        setJobs({ listScrollTop: el.scrollTop });
      });
    };
    el.addEventListener('scroll', onScroll, { passive: true });
    return () => {
      el.removeEventListener('scroll', onScroll);
      if (rafId !== null) cancelAnimationFrame(rafId);
    };
  }, [setJobs]);

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
    // Single column until the CARD (not the viewport) is wide enough for two
    // panes, then a row. `@2xl` = 42rem: the card reaches it at a ~1100px window
    // and still yields 280px of list + ~434px of detail, both usable. `@3xl`
    // (48rem) needed a 1200px window — 1340px at large text scale — which
    // pushed most narrow-window users into one column, the very complaint this
    // redesign started from. The parent results card carries `@container`; a
    // viewport `md:` here fired even when the expanded sidebar left the card
    // ~390px wide (docs/PATTERNS.md §15).
    // w-full + min-w-0: this element is the lone flex child of that card, so
    // without them its main size resolves to max-content and the panes overflow
    // (clipped by the card's overflow-hidden) instead of sharing the width.
    <div className="@2xl:flex-row flex h-full w-full min-h-0 min-w-0 flex-col">
      {/* ── Left: list pane ── */}
      {/* Single-column: hidden when a job is selected. Two-pane: always visible.
          Width is proportional-with-bounds (the utility equivalent of
          `clamp(280px, 34%, 480px)`) rather than a fixed 420/480/520px ladder:
          at the 900px window floor a fixed 420px left over ~248px for the detail
          pane, which is what clipped its action cluster. `shrink-0` keeps the
          percentage authoritative once the detail pane's content grows. */}
      <aside
        className={[
          'flex flex-col overflow-hidden border-r border-[var(--border-clear)]',
          selectedPosting ? '@2xl:flex hidden' : 'flex',
          '@2xl:w-[34%] @2xl:min-w-[280px] @2xl:max-w-[480px] @2xl:shrink-0',
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
          <Button variant="primary" onClick={onShowMore} loading={scraping}>
            {!scraping && <Plus size={12} />}
            {t('jobs.showMore')}
          </Button>
        </div>
      </aside>

      {/* ── Right: detail pane ── */}
      {/* Single-column: shown only when a job is selected. Two-pane: always. */}
      <section
        className={[
          'flex-col overflow-hidden min-w-0 flex-1',
          selectedPosting ? 'flex' : '@2xl:flex hidden',
        ].join(' ')}
      >
        {/* Controls bar — Back button for the single-column layout only */}
        {selectedPosting && (
          <div className="@2xl:hidden flex shrink-0 items-center gap-2 border-b border-[var(--border-clear)] py-2 pl-5 pr-0">
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
