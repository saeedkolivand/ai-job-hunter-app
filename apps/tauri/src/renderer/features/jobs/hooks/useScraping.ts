import { useCallback, useEffect, useRef, useState } from 'react';

import type { DATE_FILTER_OPTIONS } from '@ajh/shared';
import type { useNotification } from '@ajh/ui';

import { AUTH_BENEFITS } from '@/features/jobs/constants';
import type { Posting } from '@/features/jobs/types';
import {
  fetchJob,
  useBoardConnect,
  useBoardDisconnect,
  useBoardStatus,
  useCancelJob,
  useClearPostings,
  useLinkedInConnect,
  useLinkedInDisconnect,
  useLinkedInStatus,
  useScrapeBoard,
} from '@/services';

type ScrapeOutcome = { ok: boolean; note?: string };

interface ScrapeForm {
  board: string;
  query: string;
  location: string;
  countryCode?: string;
  latitude?: number;
  longitude?: number;
  radiusKm: number;
  pages: number;
  dateFilter: '' | (typeof DATE_FILTER_OPTIONS)[number];
  locale: string;
}

export function useScraping(notify: ReturnType<typeof useNotification>, scrapeForm: ScrapeForm) {
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

  const scrapeBoard = useScrapeBoard();
  const cancelJob = useCancelJob();
  const clearPostings = useClearPostings();

  const isLinkedInBoard = scrapeForm.board === 'linkedin';
  const linkedInStatus = useLinkedInStatus();
  const boardStatus = useBoardStatus(
    AUTH_BENEFITS.has(scrapeForm.board) && !isLinkedInBoard ? scrapeForm.board : ''
  );
  const linkedInConnect = useLinkedInConnect();
  const linkedInDisconnect = useLinkedInDisconnect();
  const boardConnect = useBoardConnect();
  const boardDisconnect = useBoardDisconnect();

  const boardConnected = isLinkedInBoard
    ? ((linkedInStatus.data as { connected?: boolean } | undefined)?.connected ?? false)
    : ((boardStatus.data as { connected?: boolean } | undefined)?.connected ?? false);
  const connectPending = isLinkedInBoard ? linkedInConnect.isPending : boardConnect.isPending;
  const disconnectPending = isLinkedInBoard
    ? linkedInDisconnect.isPending
    : boardDisconnect.isPending;

  const handleInlineDisconnect = async () => {
    try {
      if (isLinkedInBoard) await linkedInDisconnect.mutateAsync();
      else await boardDisconnect.mutateAsync(scrapeForm.board);
    } catch (err) {
      notify(err instanceof Error ? err.message : 'Disconnect failed.', 'error');
    }
  };

  const handleInlineConnect = async () => {
    try {
      const result = isLinkedInBoard
        ? await linkedInConnect.mutateAsync()
        : await boardConnect.mutateAsync(scrapeForm.board);
      const res = result as { connected?: boolean; error?: string } | undefined;
      if (res?.error) notify(res.error, 'error');
      else if (!res?.connected) {
        const boardName = isLinkedInBoard ? 'LinkedIn' : scrapeForm.board;
        notify(`${boardName} sign-in was cancelled or timed out.`, 'warning');
      }
    } catch (err) {
      notify(err instanceof Error ? err.message : 'Connection failed.', 'error');
    }
  };

  const doScrape = async () => {
    const res = (await scrapeBoard.mutateAsync({
      board: scrapeForm.board,
      query: scrapeForm.query,
      ...(scrapeForm.location ? { location: scrapeForm.location } : {}),
      ...(scrapeForm.countryCode ? { countryCode: scrapeForm.countryCode } : {}),
      ...(scrapeForm.latitude != null ? { latitude: scrapeForm.latitude } : {}),
      ...(scrapeForm.longitude != null ? { longitude: scrapeForm.longitude } : {}),
      ...(scrapeForm.radiusKm > 0 ? { radiusKm: scrapeForm.radiusKm } : {}),
      pages: scrapeForm.pages,
      ...(scrapeForm.dateFilter ? { dateFilter: scrapeForm.dateFilter } : {}),
      ...(scrapeForm.board === 'indeed' ? { locale: scrapeForm.locale } : {}),
    } as Parameters<typeof scrapeBoard.mutateAsync>[0])) as { jobId: string; error?: string };
    return res;
  };

  const startScrape = async () => {
    setScrapeOutcome(null);
    setScraping(true);
    setLivePostings([]);
    pendingFinishRef.current.clear();

    // When the search target changes, drop the previously scraped jobs so the
    // list reflects the new query rather than accumulating across searches.
    const signature = [
      scrapeForm.board,
      scrapeForm.query.trim().toLowerCase(),
      scrapeForm.location.trim().toLowerCase(),
      scrapeForm.dateFilter ?? '',
    ].join('|');
    if (signature !== lastSearchRef.current) {
      lastSearchRef.current = signature;
      try {
        await clearPostings.mutateAsync();
      } catch {
        // Best-effort — proceed with the scrape even if clearing failed.
      }
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
      const res = await doScrape();

      if (res.error) {
        setScraping(false);
        setScrapeOutcome({ ok: false, note: res.error });
        notify(res.error, 'error');
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
      notify(err instanceof Error ? err.message : 'Scraping failed.', 'error');
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
    scrapeOutcome,
    livePostings,
    setLivePostings,
    scrapeJobRef,
    startScrape,
    cancelScrape,
    handleInlineConnect,
    handleInlineDisconnect,
    boardConnected,
    connectPending,
    disconnectPending,
  };
}
