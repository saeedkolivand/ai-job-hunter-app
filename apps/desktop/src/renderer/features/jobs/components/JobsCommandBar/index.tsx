import { LayoutList, LayoutPanelLeft, ListFilter, Loader2, Plus, Trash2 } from 'lucide-react';

import type { BoardScrapeSummary } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { Button, Dropdown, Input, SegmentedControl, Tag } from '@ajh/ui';

import { BoardSummaryChips } from '@/components/scrape/BoardSummaryChips';
import { useSessionStore } from '@/store/session-store';

interface JobsCommandBarProps {
  /** Postings currently visible (numerator of the "N / M" count). */
  shownCount: number;
  /** Distinct postings before the text filter / hide-agency (denominator). */
  totalCount: number;
  /** True while a scrape job is running — swaps in the live status strip. */
  scraping: boolean;
  /** Boards-done/total fraction (0..1); null until the first board completes. */
  scrapeProgress?: number | null;
  /** Whether the "Clear" destructive action is offered at all. */
  canClear: boolean;
  onClear: () => void;
  onScrape: () => void;
  onCancelScrape: () => void;
  /**
   * Per-board outcome of the last scrape, ALREADY gated by the caller — the
   * empty state (JobsResults) is the sole owner of the explanation when there
   * are zero results, so the page passes `[]` in that case.
   */
  boardSummaries: BoardScrapeSummary[];
  /** Sanitized note for an outright scrape failure; gated like `boardSummaries`. */
  failureNote: string | null;
}

/**
 * Compact command bar that replaced the Jobs hero (title + subtitle + inline
 * form). One wrapping row of controls, plus two conditional thin rows below it:
 * the active-filter chips and the live scrape status.
 *
 * Responsive contract: the control row is `flex-wrap` with no `shrink-0`
 * no-wrap container, so at the 900px window floor the trailing action group
 * wraps onto a second line instead of being clipped. The bar is `@container`
 * so its children can respond to the page column's width rather than the
 * viewport (`docs/PATTERNS.md` §15).
 *
 * Filter/sort/view state is read straight from the session store (the same
 * source `JobsPage` and `JobsResults` read) so the page doesn't have to thread
 * eight extra props through.
 */
export function JobsCommandBar({
  shownCount,
  totalCount,
  scraping,
  scrapeProgress,
  canClear,
  onClear,
  onScrape,
  onCancelScrape,
  boardSummaries,
  failureNote,
}: JobsCommandBarProps) {
  const { t } = useTranslation();
  const { jobs, setJobs } = useSessionStore();
  const { filter, sortBy, viewMode, hideAgency } = jobs;

  const trimmedFilter = filter.trim();
  const searchChipLabel = t('jobs.filters.searchChip', { query: trimmedFilter });
  const hasFilterChips = trimmedFilter.length > 0 || hideAgency;
  const showChipsRow = hasFilterChips || boardSummaries.length > 0 || failureNote !== null;

  const clearAllFilters = () => setJobs({ filter: '', hideAgency: false });

  return (
    <div
      data-testid={TEST_IDS.jobs.commandBar}
      role="group"
      aria-label={t('jobs.commandBar.label')}
      className="@container shrink-0 px-10 pb-3 pt-6"
    >
      {/* Control row — wraps; never a fixed no-wrap strip. */}
      <div className="flex flex-wrap items-center gap-2">
        <h1 className="text-gradient shrink-0 text-lg font-bold tracking-tight">
          {t('jobs.title')}
        </h1>

        <Input
          id="jobs-filter-query"
          name="jobs-filter-query"
          prefix={<ListFilter size={12} />}
          value={filter}
          onChange={(e) => setJobs({ filter: e.target.value })}
          placeholder={t('jobs.searchPlaceholder')}
          aria-label={t('jobs.searchPlaceholder')}
          className="text-foreground/75 placeholder:text-foreground/30"
          variant="default"
          wrapperClassName="min-w-[8rem] max-w-[18rem] grow basis-44"
          allowClear
        />

        <Dropdown
          options={[
            { value: 'newest', label: t('jobs.sortNewest') },
            { value: 'oldest', label: t('jobs.sortOldest') },
            { value: 'company', label: t('jobs.sortCompany') },
          ]}
          value={sortBy}
          onChange={(value) => setJobs({ sortBy: value as 'newest' | 'oldest' | 'company' })}
          placeholder={t('jobs.sort')}
        />

        <span data-testid={TEST_IDS.jobs.hideAgencyToggle} className="inline-flex">
          <Tag.CheckableTag checked={hideAgency} onChange={(v) => setJobs({ hideAgency: v })}>
            {t('jobs.filters.hideAgency')}
          </Tag.CheckableTag>
        </span>

        {/* Count — the visible "N / M" stays the terse glanceable form; the
            accessible name spells it out for AT. */}
        <span
          className="shrink-0 tabular-nums text-[11px] text-foreground/50"
          aria-label={t('jobs.commandBar.shownCount', {
            shown: String(shownCount),
            total: String(totalCount),
          })}
        >
          {shownCount} / {totalCount}
        </span>

        {/* Trailing actions — right-aligned when there is room, wrapped to their
            own line when there isn't. */}
        <div className="ml-auto flex items-center gap-2">
          {/* View mode toggle — SegmentedControl (WAI-ARIA radiogroup + roving arrow keys) */}
          <SegmentedControl
            ariaLabel={t('jobs.viewMode')}
            value={viewMode}
            onChange={(v) => setJobs({ viewMode: v })}
            options={[
              { value: 'list', label: <LayoutList size={13} />, title: t('jobs.viewList') },
              {
                value: 'split',
                label: <LayoutPanelLeft size={13} />,
                title: t('jobs.viewSplit'),
              },
            ]}
            tone="brand"
            size="sm"
          />
          {canClear && (
            <Button variant="ghost" onClick={onClear} title={t('jobs.clearScrapedJobs')}>
              <Trash2 size={12} />
              {t('jobs.clear')}
            </Button>
          )}
          <Button variant="primary" onClick={onScrape}>
            <Plus size={12} />
            {t('jobs.scrapeJobs')}
          </Button>
        </div>
      </div>

      {/* Live scrape status — the scrape form now lives in a drawer that closes
          as soon as results stream in, so progress + cancel have to stay
          reachable here. */}
      {scraping && (
        <div
          data-testid={TEST_IDS.jobs.scrapeStatusStrip}
          role="status"
          aria-live="polite"
          className="mt-2 flex flex-wrap items-center gap-2 text-[11px] text-foreground/55"
        >
          <Loader2 size={11} aria-hidden="true" className="animate-spin text-brand-soft" />
          <span>
            {scrapeProgress == null
              ? t('jobs.scraping')
              : t('jobs.scanningPercent', { percent: Math.round(scrapeProgress * 100) })}
          </span>
          <Button variant="ghost" onClick={onCancelScrape}>
            {t('jobs.cancel')}
          </Button>
        </div>
      )}

      {/* Applied filters + last-scrape diagnostics — only when there is
          something to show. */}
      {showChipsRow && (
        <div
          data-testid={TEST_IDS.jobs.filterChips}
          role="group"
          aria-label={t('jobs.filters.activeLabel')}
          className="mt-2 flex flex-wrap items-center gap-1.5"
        >
          {trimmedFilter.length > 0 && (
            <Tag
              color="processing"
              closable
              closeLabel={t('jobs.filters.remove', { name: searchChipLabel })}
              onClose={() => setJobs({ filter: '' })}
              className="max-w-[16rem] text-[10px] font-normal"
            >
              <span className="truncate">{searchChipLabel}</span>
            </Tag>
          )}
          {hideAgency && (
            <Tag
              color="processing"
              closable
              closeLabel={t('jobs.filters.remove', { name: t('jobs.filters.hideAgency') })}
              onClose={() => setJobs({ hideAgency: false })}
              className="text-[10px] font-normal"
            >
              {t('jobs.filters.hideAgency')}
            </Tag>
          )}
          {hasFilterChips && (
            <Button variant="ghost" onClick={clearAllFilters}>
              {t('jobs.filters.clearAll')}
            </Button>
          )}
          {boardSummaries.length > 0 && <BoardSummaryChips summaries={boardSummaries} />}
          {failureNote !== null && (
            <p role="status" aria-live="polite" className="text-[11px] text-red-400/80">
              {t('jobs.lastScrapeFailed', { reason: failureNote })}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
