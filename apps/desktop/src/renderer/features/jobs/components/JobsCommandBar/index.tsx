import { LayoutList, LayoutPanelLeft, ListFilter, Loader2, Plus, Trash2 } from 'lucide-react';
import type { Ref } from 'react';

import { type BoardScrapeSummary, WORK_TYPE_OPTIONS } from '@ajh/shared';
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
  /**
   * Forwarded to the Scrape button. It is the only ALWAYS-mounted control that
   * opens the scrape drawer, so the drawer uses it as its focus-return fallback
   * when the empty-state CTA it was opened from has since unmounted.
   */
  scrapeButtonRef?: Ref<HTMLButtonElement>;
  /**
   * Whether at least one currently-displayed posting declares a `workType`
   * (computed by the caller on the exact array the work-type filter itself
   * runs over, so the two can never disagree). Gates the work-type control's
   * visibility: most searches (Greenhouse/Adzuna/…) declare nothing, and
   * three permanently-inert toggles cost a 3rd wrapped line at the 900×600
   * floor for no payoff. An ACTIVE selection always overrides this and keeps
   * the control visible regardless — see the render below.
   */
  hasDeclaredWorkType: boolean;
}

/**
 * Compact command bar that replaced the Jobs hero (title + subtitle + inline
 * form). One wrapping row of controls, plus two conditional thin rows below it:
 * the live scrape status and the active-filter chips.
 *
 * Responsive contract: the control row is `flex-wrap` with no `shrink-0`
 * no-wrap container, so at the 900px window floor the trailing action group
 * wraps onto a second line instead of being clipped. HEIGHT is the scarce
 * resource there — German runs ~30% longer and pushed this bar to 56% of the
 * content column — so every row below the first stays strictly single-line: the
 * chips row scrolls sideways instead of wrapping, and a filter that is already
 * visible as a control (hide-agency, 40px above) gets no chip at all.
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
  scrapeButtonRef,
  hasDeclaredWorkType,
}: JobsCommandBarProps) {
  const { t } = useTranslation();
  // One selector per field (see JobsPage): an unselected `useSessionStore()`
  // re-renders this bar on every mutation of every other slice.
  const setJobs = useSessionStore((s) => s.setJobs);
  const filter = useSessionStore((s) => s.jobs.filter);
  const sortBy = useSessionStore((s) => s.jobs.sortBy);
  const viewMode = useSessionStore((s) => s.jobs.viewMode);
  const hideAgency = useSessionStore((s) => s.jobs.hideAgency);
  const workTypes = useSessionStore((s) => s.jobs.workTypes);

  const trimmedFilter = filter.trim();
  const hasActiveFilter = trimmedFilter.length > 0;
  const showChipsRow = hasActiveFilter || boardSummaries.length > 0 || failureNote !== null;

  // Show the work-type control only when it can do something: some visible
  // posting declares a workType, OR the user already has a selection active.
  // The second half is load-bearing, not a nicety — without it, running a new
  // search whose results declare nothing while a selection from a PREVIOUS
  // search is still applied would hide the only control that explains/clears
  // an active filter. `matchesWorkTypeFilter`'s keep-undeclared policy means
  // that surviving selection is inert here (it drops nothing), but inert is
  // not the same as invisible — the user must still be able to see and clear it.
  const showWorkTypeFilter = hasDeclaredWorkType || workTypes.length > 0;

  // The row is shared: it holds removable FILTER chips and/or read-only scrape
  // DIAGNOSTICS. Announcing "Active filters" over a row that only carries
  // diagnostics misdescribes it, so the label follows the contents.
  const chipsRowLabel = hasActiveFilter
    ? t('jobs.filters.activeLabel')
    : t('jobs.commandBar.statusRow');

  // Single always-mounted live region (below). A `role="status"` node that is
  // created at the same moment as its text is unreliably announced by NVDA/JAWS
  // — the region has to exist BEFORE the content lands in it. This matters more
  // than usual here: the scraping strip is the only Cancel affordance once the
  // drawer closes, so a user who never hears it never learns it exists.
  const liveStatus = scraping
    ? scrapeProgress == null
      ? t('jobs.scanning')
      : t('jobs.scanningPercent', { percent: Math.round(scrapeProgress * 100) })
    : failureNote !== null
      ? t('jobs.lastScrapeFailed', { reason: failureNote })
      : '';

  return (
    <div
      data-testid={TEST_IDS.jobs.commandBar}
      role="group"
      aria-label={t('jobs.commandBar.label')}
      className="shrink-0 px-10 pb-3 pt-6"
    >
      {/* Always-mounted live region — see `liveStatus`. Kept empty when there is
          nothing to say; the visual surfaces below are aria-hidden so a change
          is announced exactly once. */}
      <div
        data-testid={TEST_IDS.jobs.scrapeStatusLive}
        role="status"
        aria-live="polite"
        className="sr-only"
      >
        {liveStatus}
      </div>

      {/* Control row — wraps; never a fixed no-wrap strip. */}
      <div className="flex flex-wrap items-center gap-2">
        <h1 className="text-body-strong shrink-0 text-foreground/90">{t('jobs.title')}</h1>

        <Input
          id="jobs-filter-query"
          name="jobs-filter-query"
          prefix={<ListFilter size={12} />}
          value={filter}
          onChange={(e) => setJobs({ filter: e.target.value })}
          placeholder={t('jobs.searchPlaceholder')}
          // A short name, not the placeholder — the placeholder is an
          // instruction ("Filter by title, company, location…") and reads as
          // one in a rotor's control list.
          aria-label={t('jobs.commandBar.filterLabel')}
          className="text-foreground/75 placeholder:text-foreground/30"
          variant="default"
          wrapperClassName="min-w-[8rem] max-w-[18rem] grow basis-44"
          allowClear
        />

        {/* Count sits with the input it describes — alone on the right it
            stranded a void of whitespace once the action group wrapped away. */}
        <span className="shrink-0 tabular-nums text-[11px] text-foreground/50">
          <span aria-hidden="true">
            {shownCount} / {totalCount}
          </span>
          {/* `aria-label` on a bare span is a prohibited-and-dropped mapping, so
              the spelled-out form is real (visually hidden) text instead. */}
          <span className="sr-only">
            {t('jobs.commandBar.shownCount', {
              shown: String(shownCount),
              total: String(totalCount),
            })}
          </span>
        </span>

        <Dropdown
          options={[
            { value: 'newest', label: t('jobs.sortNewest') },
            { value: 'oldest', label: t('jobs.sortOldest') },
            { value: 'company', label: t('jobs.sortCompany') },
          ]}
          value={sortBy}
          onChange={(value) => setJobs({ sortBy: value as 'newest' | 'oldest' | 'company' })}
          placeholder={t('jobs.sort')}
          aria-label={t('jobs.sort')}
        />

        <span data-testid={TEST_IDS.jobs.hideAgencyToggle} className="inline-flex">
          <Tag.CheckableTag checked={hideAgency} onChange={(v) => setJobs({ hideAgency: v })}>
            {t('jobs.filters.hideAgency')}
          </Tag.CheckableTag>
        </span>

        {/* View-only work-type filter — filters postings ALREADY on screen, no
            re-scrape. Same "already a visible control, no separate chip" rule
            as hideAgency above (see the chips-row test for why). Hidden when
            nothing on screen declares a workType AND no selection is active
            (`showWorkTypeFilter` above) — most searches declare nothing, and
            three permanently-inert toggles cost a 3rd wrapped line at the
            900×600 floor for zero payoff. An active selection always keeps
            its own control visible, even once it becomes inert, so it is
            never a filter the user can't see or clear. */}
        {showWorkTypeFilter && (
          <span
            role="group"
            aria-label={t('jobs.workType.label')}
            className="inline-flex items-center gap-1"
          >
            {/* Empty set silently means "any" — three identically-unchecked
                tags read as broken/unset otherwise. Same visible microcopy
                idiom as the manual-search and autopilot-wizard controls. */}
            {workTypes.length === 0 && (
              <span className="text-[10px] text-foreground/35">{t('jobs.workType.any')}</span>
            )}
            {WORK_TYPE_OPTIONS.map((opt) => (
              <Tag.CheckableTag
                key={opt}
                checked={workTypes.includes(opt)}
                onChange={(checked) =>
                  setJobs({
                    workTypes: checked ? [...workTypes, opt] : workTypes.filter((w) => w !== opt),
                  })
                }
              >
                {t(`jobs.workType.${opt}`)}
              </Tag.CheckableTag>
            ))}
          </span>
        )}

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
          <Button ref={scrapeButtonRef} variant="primary" onClick={onScrape}>
            <Plus size={12} />
            {t('jobs.scrapeJobs')}
          </Button>
        </div>
      </div>

      {/* Live scrape status — the scrape form now lives in a drawer that closes
          on Search, so progress + cancel have to stay reachable here. Same
          scanning copy as the results skeleton, so the two never disagree.
          `/70`, not `/55`: the light-scheme legibility remap in utilities.css
          lifts only /20…/50, so /55 renders LIGHTER than /50 and measured
          3.67:1 — under AA — on the one row carrying the only Cancel control. */}
      {scraping && (
        <div
          data-testid={TEST_IDS.jobs.scrapeStatusStrip}
          className="mt-2 flex flex-wrap items-center gap-2 text-[11px] text-foreground/70"
        >
          <Loader2 size={11} aria-hidden="true" className="animate-spin text-brand-soft" />
          {/* The TEXT is mirrored into the always-mounted live region above, so
              it is hidden here — otherwise the same update is announced twice.
              The Cancel button stays exposed: it is a control, not status. */}
          <span aria-hidden="true">
            {scrapeProgress == null
              ? t('jobs.scanning')
              : t('jobs.scanningPercent', { percent: Math.round(scrapeProgress * 100) })}
          </span>
          <Button variant="ghost" onClick={onCancelScrape}>
            {t('jobs.cancel')}
          </Button>
        </div>
      )}

      {/* Applied filters + last-scrape diagnostics — one strictly single-line row
          that scrolls sideways rather than wrapping, so diagnostics can never
          grow the bar and squeeze the results list at the 900×600 floor.
          Because it scrolls, the container itself must be a tab stop: content
          past the right edge is otherwise unreachable by keyboard (only the
          leftmost chip's × is focusable, which is also why axe's
          scrollable-region-focusable rule stays silent here — one focusable
          descendant satisfies it). A focusable div gets no ring by default, so
          the focus-visible style is explicit. */}
      {showChipsRow && (
        <div
          data-testid={TEST_IDS.jobs.filterChips}
          tabIndex={0}
          role="group"
          aria-label={chipsRowLabel}
          className="scrollbar-thin mt-2 flex flex-nowrap items-center gap-1.5 overflow-x-auto rounded pb-0.5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/50 focus-visible:ring-offset-1 focus-visible:ring-offset-transparent"
        >
          {trimmedFilter.length > 0 && (
            <Tag
              color="processing"
              closable
              closeLabel={t('jobs.filters.remove', { name: trimmedFilter })}
              onClose={() => setJobs({ filter: '' })}
              className="max-w-[16rem] shrink-0 text-[10px] font-normal"
            >
              <span className="truncate">
                {t('jobs.filters.searchChip', { query: trimmedFilter })}
              </span>
            </Tag>
          )}
          {boardSummaries.length > 0 && (
            <BoardSummaryChips summaries={boardSummaries} className="shrink-0 flex-nowrap" />
          )}
          {failureNote !== null && (
            // Mirrored into the always-mounted live region above, so hidden here
            // — a live region that mounts WITH its text is unreliably announced.
            <p
              aria-hidden="true"
              className="shrink-0 whitespace-nowrap text-[11px] text-red-400/80"
            >
              {t('jobs.lastScrapeFailed', { reason: failureNote })}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
