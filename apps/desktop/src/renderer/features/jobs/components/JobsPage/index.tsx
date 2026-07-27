import { useEffect, useMemo, useRef, useState } from 'react';

import {
  AGGREGATOR_BOARD_ID,
  type BoardScrapeSummary,
  type DATE_FILTER_OPTIONS,
} from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { ConfirmModal, Drawer, useNotification } from '@ajh/ui';

import { PageTransition } from '@/components/layout/PageTransition';
import { sanitizeReason } from '@/components/scrape/BoardSummaryChips';
import { JobsCommandBar } from '@/features/jobs/components/JobsCommandBar';
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

  const { jobs } = useSessionStore();
  const { filter, sortBy, hideAgency } = jobs;
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

  // `absorbedInto` traces which live-stream ids collapsed into which survivor id
  // (boards stream at different speeds and the persisted refetch can land under a
  // DIFFERENT incumbent than the one currently selected) — JobsResults consumes it
  // to re-point a stale `selectedId` at its survivor instead of silently falling
  // back to the top of the list.
  const { allPostings, absorbedInto } = useMemo(() => {
    const absorbedInto = new Map<string, string>();
    const allPostings = mergePostings(postings, livePostings, absorbedInto);
    return { allPostings, absorbedInto };
  }, [postings, livePostings]);

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

    // Cross-board clustering (ADR-029): show one row per cluster — the canonical
    // member. Non-canonical members (clusterCanonical === false) collapse into
    // it; unannotated rows (no clusterId → clusterCanonical undefined) ALWAYS
    // show — live-streamed rows are unclustered until the completion refetch.
    result = result.filter((p) => p.clusterCanonical !== false);

    // Optional agency filter — hide recruiting/staffing-agency postings.
    if (hideAgency) result = result.filter((p) => !p.isAgency);

    // Stable, deterministic ordering (audit quick win 8): an `id` tiebreak so
    // equal timestamps never reorder between renders (nondeterministic order
    // reads as flakiness), and — for the date sorts — undated postings (no
    // `postedAt`) collect in a trailing band instead of interleaving with
    // genuinely-dated ones via the `capturedAt` fallback (a scrape-time clock,
    // not a posting date, so a just-captured undated row would otherwise jump
    // above a real week-old posting).
    // Ordinal (not localeCompare) — ids are opaque keys, not display text;
    // collation can canonically-equate distinct sequences.
    const byId = (x: Posting, y: Posting) => (x.id < y.id ? -1 : x.id > y.id ? 1 : 0);
    result = [...result].sort((a, b) => {
      if (sortBy === 'company') {
        return a.company.localeCompare(b.company) || byId(a, b);
      }
      // newest / oldest: dated band first, undated (postedAt-less) band last.
      const aDated = typeof a.postedAt === 'number';
      const bDated = typeof b.postedAt === 'number';
      if (aDated !== bDated) return aDated ? -1 : 1;
      const aTime = a.postedAt ?? a.capturedAt;
      const bTime = b.postedAt ?? b.capturedAt;
      const cmp = sortBy === 'oldest' ? aTime - bTime : bTime - aTime;
      return cmp || byId(a, b);
    });

    return result;
  }, [allPostings, filter, sortBy, hideAgency]);

  // Denominator for the "N / M" count = distinct jobs (clusters): rows that
  // aren't a collapsed non-canonical duplicate. The numerator (`filtered`)
  // reduces this by the text filter + hideAgency, so both count against
  // distinct jobs, not raw postings.
  const distinctCount = useMemo(
    () => allPostings.filter((p) => p.clusterCanonical !== false).length,
    [allPostings]
  );

  const resumeId = useDefaultResumeId();

  // Persistent per-board outcome — survives the drawer auto-closing so the user
  // can always see what each board did on the last scrape. Gated on results
  // being present: when there are ZERO results the empty state (JobsResults) is
  // the SOLE owner of the explanation — without this gate both would render at
  // once. The same gate covers the outright-failure note (which has no
  // per-board summaries to chip).
  const showDiagnostics = !scraping && filtered.length > 0;

  return (
    <MatchScoresProvider resumeId={resumeId}>
      <PageTransition className="flex h-full flex-col overflow-hidden">
        {/* Centered column: constrains content to max-w-6xl on large displays,
            matching the dashboard. Both the command bar and the results area
            sit inside so they stay visually aligned. */}
        <div className="mx-auto flex w-full min-h-0 flex-1 flex-col max-w-6xl 2xl:max-w-7xl">
          {/* Command bar — replaces the old hero + inline form. It is `shrink-0`
              with NO overflow of its own (the previous bounded `overflow-y-auto`
              wrapper is what produced the stray horizontal scrollbar), so the
              results area below owns the full remaining height. */}
          <JobsCommandBar
            shownCount={filtered.length}
            totalCount={distinctCount}
            scraping={scraping}
            scrapeProgress={scrapeProgress}
            canClear={allPostings.length > 0 && !scraping}
            onClear={() => setConfirmClear(true)}
            onScrape={() => setShowScrapeForm(true)}
            onCancelScrape={cancelScrape}
            boardSummaries={showDiagnostics ? lastSummaries : []}
            failureNote={showDiagnostics ? lastFailureNote : null}
          />

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
            absorbedInto={absorbedInto}
            onShowMore={handleShowMore}
            onScrape={() => setShowScrapeForm(true)}
          />
        </div>
      </PageTransition>

      {/* New Scrape — a right slide-over instead of an inline block, so the form
          can't push the results list (or its own Start button) off screen at the
          900x600 window floor. Batch-apply is unchanged: edits only take effect
          when the user hits Search. The drawer body owns the scroll. */}
      <Drawer
        open={showScrapeForm}
        onClose={() => setShowScrapeForm(false)}
        ariaLabel={t('jobs.newScrape')}
      >
        <div
          data-testid={TEST_IDS.jobs.scrapeFormScroll}
          className="min-h-0 flex-1 overflow-y-auto p-4"
        >
          <ScrapeForm
            show={showScrapeForm}
            form={scrapeForm}
            scraping={scraping}
            scrapeOutcome={scrapeOutcome}
            onToggle={() => setShowScrapeForm(false)}
            onFormChange={(updates) => setScrapeForm({ ...scrapeForm, ...updates })}
            onStart={handleStartScrape}
            onCancel={cancelScrape}
            onGeocode={geocodeSuggest}
          />
        </div>
      </Drawer>

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
