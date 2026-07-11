import { LayoutList, LayoutPanelLeft, ListFilter, Plus, Trash2 } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';

import {
  AGGREGATOR_BOARD_ID,
  type BoardScrapeSummary,
  type DATE_FILTER_OPTIONS,
} from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, ConfirmModal, Dropdown, Input, SegmentedControl, useNotification } from '@ajh/ui';

import { PageHeader } from '@/components/layout/PageHeader';
import { PageTransition } from '@/components/layout/PageTransition';
import { BoardSummaryChips, sanitizeReason } from '@/components/scrape/BoardSummaryChips';
import { JobsResults } from '@/features/jobs/components/JobsResults';
import { ScrapeForm } from '@/features/jobs/components/ScrapeForm';
import type { ScrapeFormState } from '@/features/jobs/components/ScrapeForm/constants';
import { useScraping } from '@/features/jobs/hooks/useScraping';
import { mergePostings } from '@/features/jobs/lib/merge-postings';
import { MatchScoresProvider } from '@/features/jobs/providers';
import type { JobEvent, Posting } from '@/features/jobs/types';
import { useFormatRelativeTime } from '@/hooks/use-format-relative-time';
import { useDefaultResumeId } from '@/hooks/useDefaultResumeId';
import {
  useClearPostings,
  useGeocodeSuggest,
  useInvalidatePostings,
  useJobEvents,
  useJobPreferences,
  usePostings,
} from '@/services';
import { useSessionStore } from '@/store/session-store';

export function JobsPage() {
  const { t } = useTranslation();

  const formatRelativeTime = useFormatRelativeTime(t);

  const geocodeSuggest = useGeocodeSuggest();
  const notify = useNotification();
  const { data: postingsData = [] } = usePostings();
  const postings = postingsData as Posting[];
  const clearPostings = useClearPostings();
  const invalidatePostings = useInvalidatePostings();

  const { jobs, setJobs } = useSessionStore();
  const { filter, sortBy, viewMode } = jobs;
  const setFilter = (v: string) => setJobs({ filter: v });
  const setSortBy = (v: 'newest' | 'oldest' | 'company') => setJobs({ sortBy: v });
  const [showScrapeForm, setShowScrapeForm] = useState(false);
  const [confirmClear, setConfirmClear] = useState(false);
  // Per-board outcome of the most recent scrape. Kept in page state (not dropped
  // after reading) so the chip strip persists in the results header once the
  // form auto-closes, and the empty state can explain a zero result per board.
  const [lastSummaries, setLastSummaries] = useState<BoardScrapeSummary[]>([]);
  // An outright scrape failure (no per-board summaries exist for it) — kept
  // separately so the header/empty-state stay explainable even after the
  // dismissible form-footer note is gone. Sanitized before it ever reaches
  // state (the raw error can carry paths/URLs — same rule as chip reasons).
  const [lastFailureNote, setLastFailureNote] = useState<string | null>(null);
  const [scrapeForm, setScrapeForm] = useState<ScrapeFormState>({
    boards: [AGGREGATOR_BOARD_ID],
    query: '',
    location: '',
    radiusKm: 0,
    amount: 25,
    dateFilter: '' as '' | (typeof DATE_FILTER_OPTIONS)[number],
    companies: [],
  });

  // One-way prefill: seed the scrape location (+ its countryCode, when the saved
  // preference carries one — autopilot aggregator zero-jobs fix) from the saved
  // preferred location once it first arrives, and only if the user hasn't typed
  // one. The ref guard keeps this from re-seeding or clobbering a later user
  // edit. Picking a location here never writes back to settings.
  const { data: jobPrefs } = useJobPreferences();
  const seededLocation = useRef(false);
  useEffect(() => {
    if (seededLocation.current || !jobPrefs?.location) return;
    seededLocation.current = true;
    setScrapeForm((f) =>
      f.location
        ? f
        : { ...f, location: jobPrefs.location ?? '', countryCode: jobPrefs.countryCode }
    );
  }, [jobPrefs?.location, jobPrefs?.countryCode]);

  const {
    scraping,
    scrapeProgress,
    scrapeOutcome,
    livePostings,
    setLivePostings,
    scrapeJobRef,
    replacePendingRef,
    startScrape,
    cancelScrape,
    noteScrapeFinished,
  } = useScraping(notify, scrapeForm);

  // Throttle postings invalidation during streaming: at most one RQ refetch per
  // second so the backend cache stays the source of truth without a round-trip
  // per streamed item. This ensures remounting mid-scrape rehydrates from the
  // server cache (the query will be stale from the last tick).
  const streamInvalidateTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const throttledInvalidatePostings = useRef(invalidatePostings);
  throttledInvalidatePostings.current = invalidatePostings;

  useJobEvents((raw: unknown) => {
    const ev = raw as JobEvent;

    if (ev.type === 'job.stream') {
      const item = ev.data as Posting | undefined;
      if (
        item &&
        typeof item === 'object' &&
        'id' in item &&
        'title' in item &&
        'company' in item &&
        'url' in item
      ) {
        if (ev.jobId !== scrapeJobRef.current) return;
        if (replacePendingRef.current) {
          replacePendingRef.current = false;
          setLivePostings([item]);
          void invalidatePostings().catch(() => {}); // backend already cleared old + added this first item
        } else {
          setLivePostings((prev) => {
            if (prev.some((p) => p.id === item.id)) return prev;
            return [item, ...prev].slice(0, 500);
          });
        }
        // Throttled invalidation: keep the RQ postings cache in sync so that
        // navigating away and back mid-scrape rehydrates from the backend.
        if (!streamInvalidateTimerRef.current) {
          streamInvalidateTimerRef.current = setTimeout(() => {
            streamInvalidateTimerRef.current = null;
            void throttledInvalidatePostings.current().catch(() => {});
          }, 1000);
        }
      }
      return;
    }

    if (ev.type === 'job.completed') {
      // Read per-board summary to surface partial failures and skipped boards.
      const completedData = ev.data as { boards?: BoardScrapeSummary[] } | undefined;
      const boardSummaries = Array.isArray(completedData?.boards) ? completedData.boards : [];
      const failedBoards = boardSummaries.filter((b) => b.error);
      let note: string | undefined;
      if (failedBoards.length > 0) {
        const total = boardSummaries.length;
        const done = total - failedBoards.length;
        const failedNames = failedBoards
          .map((b) => t(`jobs.boards.${b.board}`, { defaultValue: b.board }))
          .join(', ');
        note = t('jobs.partialScrapeNote', {
          done: String(done),
          total: String(total),
          failed: failedNames,
        });
      }
      // Capture the active job id BEFORE noteScrapeFinished clears scrapeJobRef.
      // Guard: only surface diagnostics for the active scrape job — stale
      // `job.completed` events from a previous round must not overwrite the strip.
      const isActiveJob = ev.jobId === scrapeJobRef.current;
      noteScrapeFinished(ev.jobId, { ok: true, note });
      void invalidatePostings();
      if (!isActiveJob) return;
      // Persist the full per-board summaries so the chip strip surfaces WHY a
      // board returned 0 (needs-login / needs-company / needs-keys / errored /
      // truncated) — persistently, replacing the previous transient skip-toasts.
      setLastSummaries(boardSummaries);
      setLastFailureNote(null);
    } else if (ev.type === 'job.failed') {
      // Guard: `jobs:event` is a global channel — scrape, AI, autopilot, agent,
      // and pipeline jobs ALL emit `job.failed` on it. Capture isActiveJob
      // BEFORE noteScrapeFinished (which clears scrapeJobRef on a match) so an
      // unrelated background failure (e.g. an autopilot run) can't wipe the
      // strip or paint a foreign error as "Last scrape failed" — mirrors the
      // job.completed guard above.
      const isActiveJob = ev.jobId === scrapeJobRef.current;
      const raw = typeof ev.data === 'string' ? ev.data : t('jobs.scrapeFailed');
      const sanitized = sanitizeReason(raw);
      // noteScrapeFinished stays unconditional — it's internally buffered/
      // guarded by job id (a foreign jobId is simply parked, never surfaced).
      noteScrapeFinished(ev.jobId, { ok: false, note: sanitized });
      if (!isActiveJob) return;
      // The whole scrape errored — there are no per-board summaries (nothing to
      // chip), so keep a minimal sanitized failure note instead: the dismissible
      // form-footer note alone would make the failure invisible again once the
      // form closes/is dismissed.
      setLastSummaries([]);
      setLastFailureNote(sanitized);
    }
  });

  // Clear the stream-invalidation timer on unmount so it can't fire after the
  // component is gone and call a stale invalidatePostings closure.
  useEffect(() => {
    return () => {
      if (streamInvalidateTimerRef.current) {
        clearTimeout(streamInvalidateTimerRef.current);
        streamInvalidateTimerRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    if (livePostings.length > 0) setShowScrapeForm(false);
  }, [livePostings.length]);

  const allPostings = useMemo(
    () => mergePostings(postings, livePostings),
    [postings, livePostings]
  );

  const handleClearPostings = async () => {
    setConfirmClear(false);
    await clearPostings.mutateAsync();
    setLivePostings([]);
    setLastSummaries([]);
    setLastFailureNote(null);
  };

  // Start a fresh scrape — drop the previous run's chip strip/failure note so
  // neither reads as the new run's outcome (a fresh one arrives on the next
  // job.completed / job.failed).
  const handleStartScrape = () => {
    setLastSummaries([]);
    setLastFailureNote(null);
    void startScrape();
  };

  // "Show more" (#36): fetch the next batch by raising the requested job count
  // and re-scraping. The search signature is unchanged, so scraped postings are
  // kept and the extra results append (deduped by id).
  const handleShowMore = () => {
    const next = scrapeForm.amount + 25;
    setScrapeForm({ ...scrapeForm, amount: next });
    void startScrape(next);
  };

  const filtered = useMemo(() => {
    let result = allPostings;
    const q = filter.trim().toLowerCase();
    if (q) {
      result = result.filter(
        (p) =>
          p.title.toLowerCase().includes(q) ||
          p.company.toLowerCase().includes(q) ||
          (p.location ?? '').toLowerCase().includes(q)
      );
    }

    result = [...result].sort((a, b) => {
      if (sortBy === 'newest') {
        const aTime = a.postedAt ?? a.capturedAt;
        const bTime = b.postedAt ?? b.capturedAt;
        return bTime - aTime;
      } else if (sortBy === 'oldest') {
        const aTime = a.postedAt ?? a.capturedAt;
        const bTime = b.postedAt ?? b.capturedAt;
        return aTime - bTime;
      } else if (sortBy === 'company') {
        return a.company.localeCompare(b.company);
      }
      return 0;
    });

    return result;
  }, [allPostings, filter, sortBy]);

  const resumeId = useDefaultResumeId();

  return (
    <MatchScoresProvider resumeId={resumeId}>
      <PageTransition className="flex h-full flex-col overflow-hidden">
        {/* Centered column: constrains content to max-w-6xl on large displays,
            matching the dashboard. Both the pinned header and the scroll area
            sit inside so they stay visually aligned. */}
        <div className="mx-auto flex w-full min-h-0 flex-1 flex-col max-w-6xl 2xl:max-w-7xl">
          {/* Pinned header + scrape form; the list below owns the scroll. */}
          <div className="shrink-0 px-10 pt-10">
            <PageHeader
              title={t('jobs.title')}
              subtitle={t('jobs.subtitle')}
              badge={t('jobs.eyebrow')}
              actions={
                <div className="flex items-center gap-2">
                  <Button
                    variant="primary"
                    onClick={() => setShowScrapeForm(!showScrapeForm)}
                    className="transition-all duration-150 ease-out"
                  >
                    <Plus size={12} />
                    {t('jobs.scrapeJobs')}
                  </Button>
                  {allPostings.length > 0 && !scraping && (
                    <Button
                      variant="ghost"
                      onClick={() => setConfirmClear(true)}
                      title={t('jobs.clearScrapedJobs')}
                    >
                      <Trash2 size={12} />
                      {t('jobs.clear')}
                    </Button>
                  )}
                  <Input
                    id="jobs-filter-query"
                    name="jobs-filter-query"
                    prefix={<ListFilter size={12} />}
                    value={filter}
                    onChange={(e) => setFilter(e.target.value)}
                    placeholder={t('jobs.searchPlaceholder')}
                    className="text-foreground/75 placeholder:text-foreground/30"
                    variant="default"
                    wrapperClassName="w-48"
                    allowClear
                  />
                  <Dropdown
                    options={[
                      { value: 'newest', label: t('jobs.sortNewest') },
                      { value: 'oldest', label: t('jobs.sortOldest') },
                      { value: 'company', label: t('jobs.sortCompany') },
                    ]}
                    value={sortBy}
                    onChange={(value) => setSortBy(value as 'newest' | 'oldest' | 'company')}
                    placeholder={t('jobs.sort')}
                  />
                  <span className="text-[11px] text-foreground/40">
                    {filtered.length} / {allPostings.length}
                  </span>
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
                </div>
              }
            />

            <ScrapeForm
              show={showScrapeForm}
              form={scrapeForm}
              scraping={scraping}
              scrapeOutcome={scrapeOutcome}
              onToggle={() => setShowScrapeForm(!showScrapeForm)}
              onFormChange={(updates) => setScrapeForm({ ...scrapeForm, ...updates })}
              onStart={handleStartScrape}
              onCancel={cancelScrape}
              onGeocode={geocodeSuggest}
            />

            {/* Persistent per-board outcome — survives the form auto-closing so
                the user can always see what each board did on the last scrape.
                Gated on results being present: when there are ZERO results the
                empty state (JobsResults) is the SOLE owner of the explanation —
                without this gate both would render at once. */}
            {!scraping && filtered.length > 0 && lastSummaries.length > 0 && (
              <BoardSummaryChips summaries={lastSummaries} className="mb-4" />
            )}
            {/* An outright scrape failure has no per-board summaries to chip —
                keep a minimal sanitized note visible instead of going silent
                once the dismissible form-footer note is gone. Same
                results-present gating as above. */}
            {!scraping && filtered.length > 0 && lastFailureNote && (
              <p role="status" aria-live="polite" className="mb-4 text-[11px] text-red-400/80">
                {t('jobs.lastScrapeFailed', { reason: lastFailureNote })}
              </p>
            )}
          </div>

          <JobsResults
            filtered={filtered}
            formatRelativeTime={formatRelativeTime}
            scraping={scraping}
            scrapeProgress={scrapeProgress}
            boardSummaries={lastSummaries}
            failureNote={lastFailureNote}
            // Unfiltered count — lets JobsResults tell "genuinely zero
            // postings" apart from "the text filter hid everything" so the
            // empty state doesn't re-show a prior scrape's diagnostics when a
            // filter (not the scrape) is what emptied the visible list.
            totalCount={allPostings.length}
            onShowMore={handleShowMore}
            onScrape={() => setShowScrapeForm(true)}
          />
        </div>
      </PageTransition>

      <ConfirmModal
        open={confirmClear}
        onClose={() => setConfirmClear(false)}
        onConfirm={() => void handleClearPostings()}
        title={t('jobs.clearConfirmTitle')}
        description={t('jobs.clearConfirmDesc')}
        confirmText={t('jobs.clear')}
        variant="danger"
        isConfirming={clearPostings.isPending}
      />
    </MatchScoresProvider>
  );
}
