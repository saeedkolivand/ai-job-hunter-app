import { useCallback, useEffect, useRef, useState } from 'react';

import type { useNotification } from '@ajh/ui';

import type { ScrapeFormState } from '@/features/jobs/components/ScrapeForm/constants';
import type { Posting } from '@/features/jobs/types';
import { fetchJob, useCancelJob, useScrapeBoards, useScrapeProgress } from '@/services';

type ScrapeOutcome = { ok: boolean; note?: string };

export function useScraping(
  notify: ReturnType<typeof useNotification>,
  scrapeForm: ScrapeFormState
) {
  const [scraping, setScraping] = useState(false);
  const [scrapeJobId, setScrapeJobId] = useState<string | null>(null);
  const [scrapeOutcome, setScrapeOutcome] = useState<ScrapeOutcome | null>(null);
  const [livePostings, setLivePostings] = useState<Posting[]>([]);
  const scrapeJobRef = useRef<string | null>(null);
  // Completion/failure events can arrive before startScrape records the jobId
  // (the backend spawns the task and streams events before invoke resolves).
  // Buffer those here so the outcome isn't lost.
  const pendingFinishRef = useRef<Map<string, ScrapeOutcome>>(new Map());
  // Signature of the last search; a new search clears previous scraped jobs.
  const lastSearchRef = useRef<string>('');
  // Latched when the search target changes: the next scrape replaces (rather
  // than appends to) the persisted postings, applied on the first stream item.
  const replacePendingRef = useRef(false);

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
    setScrapeOutcome(null);
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
      scrapeForm.radiusKm,
      scrapeForm.dateFilter ?? '',
      scrapeForm.companies.slice().sort().join(','),
    ].join('|');
    if (signature !== lastSearchRef.current) {
      lastSearchRef.current = signature;
      replacePendingRef.current = true;
    } else {
      replacePendingRef.current = false;
    }

    const prevJobId = scrapeJobRef.current;
    if (prevJobId) {
      scrapeJobRef.current = null;
      setScrapeJobId(null);
      try {
        await cancelJob.mutateAsync(prevJobId);
      } catch {
        // best-effort
      }
    }

    try {
      const res = await doScrape(amount, replacePendingRef.current);

      if (res.error) {
        setScraping(false);
        setScrapeOutcome({ ok: false, note: res.error });
        notify.error({ message: res.error });
        return;
      }

      scrapeJobRef.current = res.jobId;
      setScrapeJobId(res.jobId);

      // If the job already finished during the invoke round-trip, apply it now.
      const buffered = pendingFinishRef.current.get(res.jobId);
      if (buffered) {
        pendingFinishRef.current.delete(res.jobId);
        finishScrape(res.jobId, buffered);
      }
    } catch (err) {
      setScraping(false);
      setScrapeOutcome({ ok: false, note: err instanceof Error ? err.message : String(err) });
      notify.error({ message: err instanceof Error ? err.message : 'Scraping failed.' });
    }
  };

  const cancelScrape = async () => {
    setScraping(false);
    setScrapeOutcome(null);
    scrapeJobRef.current = null;
    const jobId = scrapeJobId;
    setScrapeJobId(null);
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
   */
  const finishScrape = useCallback((jobId: string, outcome: ScrapeOutcome) => {
    if (!jobId || jobId !== scrapeJobRef.current) return;
    scrapeJobRef.current = null;
    setScrapeJobId(null);
    setScraping(false);
    setScrapeOutcome(outcome);
  }, []);

  /**
   * Handle a completion/failure event. If it arrives before startScrape has
   * recorded the jobId (a fast scrape can finish during the invoke round-trip),
   * buffer it so startScrape can apply it once the id is known.
   */
  const noteScrapeFinished = (jobId: string, outcome: ScrapeOutcome) => {
    if (!jobId) return;
    if (jobId === scrapeJobRef.current) finishScrape(jobId, outcome);
    else pendingFinishRef.current.set(jobId, outcome);
  };

  // Watchdog: streamed events can be dropped (async listener churn, missed
  // emits). Poll the job tracker — the authoritative terminal state — so a
  // finished scrape can never leave the UI stuck "searching".
  useEffect(() => {
    if (!scrapeJobId) return;
    let cancelled = false;
    const check = async () => {
      try {
        const job = (await fetchJob(scrapeJobId)) as { status?: string; error?: string } | null;
        if (cancelled || !job?.status) return;
        if (job.status === 'completed') finishScrape(scrapeJobId, { ok: true });
        else if (job.status === 'failed')
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
    scrapeJobRef,
    replacePendingRef,
    startScrape,
    cancelScrape,
  };
}
