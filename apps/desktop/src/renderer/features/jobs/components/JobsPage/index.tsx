import { useEffect, useMemo, useRef, useState } from 'react';

import type { BoardScrapeSummary } from '@ajh/shared';
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

  const { jobs, setJobs } = useSessionStore();
  // `scrapeForm` and the two diagnostics fields live in the session store, not
  // in this component: they describe a backend scrape that keeps running across
  // a route change (see `JobsSlice`).
  const { filter, sortBy, hideAgency, scrapeForm, scrapeSummaries, scrapeFailureNote } = jobs;
  const [showScrapeForm, setShowScrapeForm] = useState(false);
  // Always-mounted opener for the scrape drawer. The drawer normally returns
  // focus to whatever opened it, but the empty-state "Search jobs" CTA unmounts
  // the moment a scrape starts, so this is the fallback that keeps focus off
  // <body> on the first-run path.
  const scrapeButtonRef = useRef<HTMLButtonElement>(null);
  const [confirmClear, setConfirmClear] = useState(false);

  /**
   * Patch the scrape form through the store.
   *
   * Reads the LATEST slice via `getState()` rather than the render-captured
   * `scrapeForm` so two changes dispatched in the same tick (#884 — e.g. a
   * location pick firing onChange then onSelectSuggestion) compose instead of
   * the second clobbering the first. `setJobs` is a plain patch setter, so the
   * functional-update guarantee `setState` gave us has to come from here.
   */
  const patchScrapeForm = (updates: Partial<ScrapeFormState>) =>
    setJobs({ scrapeForm: { ...useSessionStore.getState().jobs.scrapeForm, ...updates } });

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
    // The ref is per-mount, but the form is not: on a remount the stored
    // location already satisfies this guard, so re-seeding is a no-op.
    const form = useSessionStore.getState().jobs.scrapeForm;
    if (form.location) return;
    setJobs({
      scrapeForm: { ...form, location: jobPrefs.location ?? '', countryCode: jobPrefs.countryCode },
    });
  }, [jobPrefs?.location, jobPrefs?.countryCode, setJobs]);

  const {
    scraping,
    scrapeProgress,
    scrapeOutcome,
    livePostings,
    setLivePostings,
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

  // Every identity check below reads the store SYNCHRONOUSLY via `getState()`
  // rather than a `useSessionStore(selector)` value captured in this closure.
  // The handler runs on backend stream events, not on React's schedule, so a
  // captured value can be a render behind — which is exactly the staleness the
  // old `scrapeJobRef` existed to prevent, and would drop stream items or
  // attribute them to the wrong job.
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
        if (ev.jobId !== useSessionStore.getState().jobs.scrapeJobId) return;
        if (useSessionStore.getState().jobs.replacePending) {
          setJobs({ replacePending: false });
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
      // Capture the active job id BEFORE noteScrapeFinished clears it.
      // Guard: only surface diagnostics for the active scrape job — stale
      // `job.completed` events from a previous round must not overwrite the strip.
      const isActiveJob = ev.jobId === useSessionStore.getState().jobs.scrapeJobId;
      noteScrapeFinished(ev.jobId, { ok: true, note });
      void invalidatePostings();
      if (!isActiveJob) return;
      // Persist the full per-board summaries so the chip strip surfaces WHY a
      // board returned 0 (needs-login / needs-company / needs-keys / errored /
      // truncated) — persistently, replacing the previous transient skip-toasts.
      setJobs({ scrapeSummaries: boardSummaries, scrapeFailureNote: null });
    } else if (ev.type === 'job.failed') {
      // Guard: `jobs:event` is a global channel — scrape, AI, autopilot, agent,
      // and pipeline jobs ALL emit `job.failed` on it. Capture isActiveJob
      // BEFORE noteScrapeFinished (which clears the stored job id on a match) so
      // an unrelated background failure (e.g. an autopilot run) can't wipe the
      // strip or paint a foreign error as "Last scrape failed" — mirrors the
      // job.completed guard above.
      const isActiveJob = ev.jobId === useSessionStore.getState().jobs.scrapeJobId;
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
      setJobs({ scrapeSummaries: [], scrapeFailureNote: sanitized });
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
    setJobs({ scrapeSummaries: [], scrapeFailureNote: null });
  };

  // Start a fresh scrape — drop the previous run's chip strip/failure note so
  // neither reads as the new run's outcome (a fresh one arrives on the next
  // job.completed / job.failed).
  //
  // Closing the drawer here, in the USER-INITIATED handler, is deliberate. It
  // used to close from an effect on `livePostings.length` — i.e. driven by a
  // background stream event, which yanked focus out from under whatever the
  // user was doing and, on the first-run path (empty state → "Search jobs" →
  // drawer), unmounted the very CTA the drawer would restore focus to, dropping
  // focus to <body>. Closing on the click also means a scrape that returns ZERO
  // results still closes the drawer instead of stranding it open (WCAG 2.4.3).
  const handleStartScrape = () => {
    setJobs({ scrapeSummaries: [], scrapeFailureNote: null });
    setShowScrapeForm(false);
    void startScrape();
  };

  // "Show more" (#36): fetch the next batch by raising the requested job count
  // and re-scraping. The search signature is unchanged, so scraped postings are
  // kept and the extra results append (deduped by id).
  const handleShowMore = () => {
    const next = scrapeForm.amount + 25;
    patchScrapeForm({ amount: next });
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
            boardSummaries={showDiagnostics ? scrapeSummaries : []}
            failureNote={showDiagnostics ? scrapeFailureNote : null}
            scrapeButtonRef={scrapeButtonRef}
          />

          <JobsResults
            filtered={filtered}
            formatRelativeTime={formatRelativeTime}
            scraping={scraping}
            scrapeProgress={scrapeProgress}
            boardSummaries={scrapeSummaries}
            failureNote={scrapeFailureNote}
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
          when the user hits Search. ScrapeForm owns the pinned header/footer and
          the single scrolling body inside the panel. */}
      <Drawer
        open={showScrapeForm}
        onClose={() => setShowScrapeForm(false)}
        ariaLabel={t('jobs.newScrape')}
        returnFocusTo={scrapeButtonRef}
      >
        <ScrapeForm
          show={showScrapeForm}
          form={scrapeForm}
          scraping={scraping}
          scrapeOutcome={scrapeOutcome}
          onToggle={() => setShowScrapeForm(false)}
          // #884: two changes dispatched in one tick (e.g. a location pick
          // firing onChange then onSelectSuggestion) must not both spread the
          // SAME captured `scrapeForm` — `patchScrapeForm` re-reads the store.
          onFormChange={(updates) => patchScrapeForm(updates)}
          onStart={handleStartScrape}
          onCancel={cancelScrape}
          onGeocode={geocodeSuggest}
        />
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
