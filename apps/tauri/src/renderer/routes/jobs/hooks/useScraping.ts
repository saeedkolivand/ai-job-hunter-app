import { useRef, useState } from 'react';

import type { DATE_FILTER_OPTIONS } from '@ajh/shared';
import type { useNotification } from '@ajh/ui';

import {
  useBoardConnect,
  useBoardDisconnect,
  useBoardStatus,
  useCancelJob,
  useLinkedInConnect,
  useLinkedInDisconnect,
  useLinkedInStatus,
  useScrapeBoard,
} from '@/services';

import { AUTH_BENEFITS } from '../constants';
import type { Posting } from '../types';

interface ScrapeForm {
  board: string;
  query: string;
  location: string;
  pages: number;
  dateFilter: '' | (typeof DATE_FILTER_OPTIONS)[number];
  locale: string;
}

export function useScraping(notify: ReturnType<typeof useNotification>, scrapeForm: ScrapeForm) {
  const [scraping, setScraping] = useState(false);
  const [scrapeJobId, setScrapeJobId] = useState<string | null>(null);
  const [scrapeOutcome, setScrapeOutcome] = useState<{ ok: boolean; note?: string } | null>(null);
  const [livePostings, setLivePostings] = useState<Posting[]>([]);
  const scrapeJobRef = useRef<string | null>(null);

  const scrapeBoard = useScrapeBoard();
  const cancelJob = useCancelJob();

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
      let res = await doScrape();

      if (res.error?.includes('Concurrency limit')) {
        if (res.jobId) {
          try {
            await cancelJob.mutateAsync(res.jobId);
          } catch {
            // best-effort
          }
        }
        res = await doScrape();
      }

      if (res.error) {
        setScraping(false);
        setScrapeOutcome({ ok: false, note: res.error });
        notify(res.error, 'error');
        return;
      }

      scrapeJobRef.current = res.jobId;
      setScrapeJobId(res.jobId);
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

  return {
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
