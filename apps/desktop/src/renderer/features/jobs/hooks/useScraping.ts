import { useCallback, useEffect, useRef, useState } from 'react';

import type { BoardScrapeSummary } from '@ajh/shared';
import type { useNotification } from '@ajh/ui';

import type { ScrapeFormState } from '@/features/jobs/components/ScrapeForm/constants';
import type { Posting, ScrapeOutcome } from '@/features/jobs/types';
import { fetchJob, useCancelJob, useScrapeBoards, useScrapeProgress } from '@/services';
import { useSessionStore } from '@/store/session-store';

export function useScraping(
  notify: ReturnType<typeof useNotification>,
  scrapeForm: ScrapeFormState
) {
  // Everything that describes the BACKEND job lives in the session store
  // (`JobsSlice`), not in this hook: the Rust scrape outlives a route change, so
  // component-local state meant navigating away orphaned an uncancellable run,
  // lost its diagnostics, and reset the replace/append bookkeeping — which then
  // made the next "Show more" wipe the persisted postings.
  const setJobs = useSessionStore((s) => s.setJobs);
  const scrapeJobId = useSessionStore((s) => s.jobs.scrapeJobId);
  const scrapeOutcome = useSessionStore((s) => s.jobs.scrapeOutcome);
  // `scraping` stays LOCAL and derived — never a stored field of its own, which
  // could desync from the job id it describes. It is seeded from the surviving
  // job id so a remount mid-scrape shows the progress strip and Cancel again;
  // the watchdog below reconciles it against the job tracker.
  const [scraping, setScraping] = useState(
    () => useSessionStore.getState().jobs.scrapeJobId !== null
  );
  // Transient display buffer only: the backend posting cache plus the throttled
  // `invalidatePostings` in JobsPage rehydrate the list on remount, so this does
  // NOT need to survive navigation.
  const [livePostings, setLivePostings] = useState<Posting[]>([]);
  // Completion/failure events can arrive before startScrape records the jobId
  // (the backend spawns the task and streams events before invoke resolves).
  // Buffer those here — a per-invoke buffer cannot outlive the call it serves,
  // so it stays local.
  const pendingFinishRef = useRef<Map<string, ScrapeOutcome>>(new Map());

  const scrapeBoards = useScrapeBoards();
  const cancelJob = useCancelJob();
  // Live boards-done/total fraction (0..1) for the in-flight scrape; null until
  // the first board completes and after the scrape ends (scrapeJobId → null).
  const scrapeProgress = useScrapeProgress(scrapeJobId);

  const doScrape = async (amount: number, replace: boolean) => {
    const res = (await scrapeBoards.mutateAsync({
      boards: scrapeForm.boards,
      query: scrapeForm.query,
      ...(scrapeForm.location ? { location: scrapeForm.location } : {}),
      ...(scrapeForm.countryCode ? { countryCode: scrapeForm.countryCode } : {}),
      ...(scrapeForm.latitude != null ? { latitude: scrapeForm.latitude } : {}),
      ...(scrapeForm.longitude != null ? { longitude: scrapeForm.longitude } : {}),
      ...(scrapeForm.radiusKm > 0 ? { radiusKm: scrapeForm.radiusKm } : {}),
      amount,
      ...(replace ? { replace: true } : {}),
      ...(scrapeForm.dateFilter ? { dateFilter: scrapeForm.dateFilter } : {}),
      ...(scrapeForm.companies.length > 0 ? { companies: scrapeForm.companies } : {}),
    } as Parameters<typeof scrapeBoards.mutateAsync>[0])) as { jobId: string; error?: string };
    return res;
  };

  const startScrape = async (amountOverride?: number) => {
    // "Show more" (#36) re-runs with a larger amount; the search signature is
    // unchanged so scraped postings are NOT cleared and the extra pages append.
    const amount = amountOverride ?? scrapeForm.amount;
    setScraping(true);
    pendingFinishRef.current.clear();

    // When the search target changes, drop the previously scraped jobs so the
    // list reflects the new query rather than accumulating across searches.
    // For "Show more" (same signature) keep existing results so the list
    // doesn't jump to the top.
    const signature = [
      [...scrapeForm.boards].sort().join(','),
      scrapeForm.query.trim().toLowerCase(),
      scrapeForm.location.trim().toLowerCase(),
      // Geo fields: a geo-different search (same keywords, different country /
      // coordinates / radius) targets a different market, so it must REPLACE the
      // stale results rather than append to them.
      scrapeForm.countryCode ?? '',
      scrapeForm.latitude ?? '',
      scrapeForm.longitude ?? '',
      scrapeForm.radiusKm ?? '',
      scrapeForm.dateFilter ?? '',
      scrapeForm.companies.slice().sort().join(','),
    ].join('|');
    // Read through getState(): the signature of the search the DISPLAYED
    // postings came from, which survives a route change. Compared against a
    // freshly-mounted `''` every navigation made "Show more" send replace:true
    // and destroy the user's result set.
    const replace = signature !== useSessionStore.getState().jobs.lastSearchSignature;
    setJobs({
      scrapeOutcome: null,
      lastSearchSignature: signature,
      replacePending: replace,
    });

    const prevJobId = useSessionStore.getState().jobs.scrapeJobId;
    if (prevJobId) {
      setJobs({ scrapeJobId: null });
      try {
        await cancelJob.mutateAsync(prevJobId);
      } catch {
        // best-effort
      }
    }

    try {
      const res = await doScrape(amount, replace);

      if (res.error) {
        setScraping(false);
        setJobs({ scrapeOutcome: { ok: false, note: res.error } });
        notify.error({ message: res.error });
        return;
      }

      setJobs({ scrapeJobId: res.jobId });

      // If the job already finished during the invoke round-trip, apply it now.
      const buffered = pendingFinishRef.current.get(res.jobId);
      if (buffered) {
        pendingFinishRef.current.delete(res.jobId);
        finishScrape(res.jobId, buffered);
      }
    } catch (err) {
      setScraping(false);
      setJobs({
        scrapeOutcome: { ok: false, note: err instanceof Error ? err.message : String(err) },
      });
      notify.error({ message: err instanceof Error ? err.message : 'Scraping failed.' });
    }
  };

  const cancelScrape = async () => {
    setScraping(false);
    // getState(), not the subscribed value: this handler can run on a mount that
    // never started the scrape it is cancelling (the user navigated away and
    // back), and it must cancel whatever is actually in flight right now.
    const jobId = useSessionStore.getState().jobs.scrapeJobId;
    setJobs({ scrapeOutcome: null, scrapeJobId: null });
    if (jobId) {
      try {
        await cancelJob.mutateAsync(jobId);
      } catch {
        // Best-effort — UI is already reset.
      }
    }
  };

  /**
   * Reset scraping state once the active job finishes/fails. Idempotent and
   * keyed by jobId, so the event path and the watchdog poll can both call it
   * safely — only the first one for the active job takes effect.
   *
   * `summaries` is written under the SAME identity guard (the watchdog recovery
   * path); the event path leaves it undefined because JobsPage owns the
   * translated per-board strip.
   */
  const finishScrape = useCallback(
    (jobId: string, outcome: ScrapeOutcome, summaries?: BoardScrapeSummary[]) => {
      if (!jobId || jobId !== useSessionStore.getState().jobs.scrapeJobId) return;
      setJobs({
        scrapeJobId: null,
        scrapeOutcome: outcome,
        ...(summaries ? { scrapeSummaries: summaries, scrapeFailureNote: null } : {}),
      });
      setScraping(false);
    },
    [setJobs]
  );

  /**
   * Handle a completion/failure event. If it arrives before startScrape has
   * recorded the jobId (a fast scrape can finish during the invoke round-trip),
   * buffer it so startScrape can apply it once the id is known.
   */
  const noteScrapeFinished = (jobId: string, outcome: ScrapeOutcome) => {
    if (!jobId) return;
    if (jobId === useSessionStore.getState().jobs.scrapeJobId) finishScrape(jobId, outcome);
    else pendingFinishRef.current.set(jobId, outcome);
  };

  // Watchdog: streamed events can be dropped (async listener churn, missed
  // emits) and are not delivered at all while JobsPage is unmounted. Poll the
  // job tracker — the authoritative terminal state — so a finished scrape can
  // never leave the UI stuck "searching". Because `scrapeJobId` now comes from
  // the store, this re-arms on remount and settles a job that ended while the
  // user was on another route.
  useEffect(() => {
    if (!scrapeJobId) return;
    let cancelled = false;
    const check = async () => {
      try {
        const job = (await fetchJob(scrapeJobId)) as {
          status?: string;
          error?: string;
          result?: { boards?: BoardScrapeSummary[] };
        } | null;
        if (cancelled || !job?.status) return;
        if (job.status === 'completed') {
          // The tracker keeps the same `{ count, boards }` payload the
          // `job.completed` EVENT carries, so a scrape that finished while the
          // page was unmounted (no subscription, no event) still gets its
          // per-board diagnostics back instead of an unexplained empty result.
          const boards = job.result?.boards;
          finishScrape(scrapeJobId, { ok: true }, Array.isArray(boards) ? boards : undefined);
        } else if (job.status === 'failed')
          finishScrape(scrapeJobId, { ok: false, note: job.error ?? undefined });
        else if (job.status === 'cancelled') finishScrape(scrapeJobId, { ok: false });
      } catch {
        // Transient — retry on the next tick.
      }
    };
    const id = setInterval(() => void check(), 2500);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [scrapeJobId, finishScrape]);

  return {
    noteScrapeFinished,
    scraping,
    scrapeJobId,
    scrapeProgress,
    scrapeOutcome,
    livePostings,
    setLivePostings,
    startScrape,
    cancelScrape,
  };
}
