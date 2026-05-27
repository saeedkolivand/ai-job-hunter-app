import { Plus, Search, Trash2 } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useMemo, useRef, useState } from 'react';
import { createFileRoute } from '@tanstack/react-router';

import type { DATE_FILTER_OPTIONS, JobInteraction } from '@ajh/shared';
import { Button, GlassCard, Input, SelectDropdown, useNotification } from '@ajh/ui';

import { PageHeader } from '@/components/layout/PageHeader';
import { PageTransition } from '@/components/layout/PageTransition';
import { ApplyDrawer } from '@/features/jobs/components/ApplyDrawer';
import { PostingRow } from '@/features/jobs/components/PostingRow';
import { ScrapeForm } from '@/features/jobs/components/ScrapeForm';
import { useTranslation } from '@/lib/i18n';
import { staggeredItem, transition } from '@ajh/ui';
import { useAppClient } from '@/providers/AppClientProvider';
import {
  useBoardConnect,
  useBoardDisconnect,
  useBoardStatus,
  useCancelJob,
  useClearPostings,
  useJobEvents,
  useLinkedInConnect,
  useLinkedInDisconnect,
  useLinkedInStatus,
  usePostings,
  useScrapeBoard,
} from '@/services';
import { useSessionStore } from '@/store/session-store';

export const Route = createFileRoute('/jobs')({ component: Jobs });

interface Posting {
  id: string;
  source: string;
  externalId: string;
  url: string;
  title: string;
  company: string;
  location?: string;
  remote?: boolean;
  description: string;
  postedAt?: number;
  capturedAt: number;
  interactions?: JobInteraction[];
}

interface JobEvent {
  type: string;
  jobId: string;
  data?: unknown;
  ts: number;
}

const AUTH_BENEFITS = new Set(['linkedin', 'indeed', 'xing']);

function Jobs() {
  const { t } = useTranslation();

  const formatRelativeTime = (timestamp?: number): string => {
    if (!timestamp) return '';
    const now = Date.now();
    const diff = now - timestamp;
    const seconds = Math.floor(diff / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);
    const days = Math.floor(hours / 24);
    const weeks = Math.floor(days / 7);
    const months = Math.floor(days / 30);

    if (minutes < 1) return t('jobs.timeJustNow');
    if (minutes < 60) return t('jobs.timeMinutesAgo', { m: minutes });
    if (hours < 24) return t('jobs.timeHoursAgo', { h: hours });
    if (days < 7) return t('jobs.timeDaysAgo', { d: days });
    if (weeks < 4) return t('jobs.timeWeeksAgo', { w: weeks });
    return t('jobs.timeMonthsAgo', { m: months });
  };

  const api = useAppClient();
  const notify = useNotification();
  const { data: postingsData = [] } = usePostings();
  const postings = postingsData as Posting[];
  const scrapeBoard = useScrapeBoard();
  const cancelJob = useCancelJob();
  const clearPostings = useClearPostings();

  const { jobs, setJobs } = useSessionStore();
  const { filter, sortBy } = jobs;
  const setFilter = (v: string) => setJobs({ filter: v });
  const setSortBy = (v: 'newest' | 'oldest' | 'company') => setJobs({ sortBy: v });
  const [active, setActive] = useState<Posting | null>(null);
  const [showScrapeForm, setShowScrapeForm] = useState(false);
  const [scrapeForm, setScrapeForm] = useState({
    board: 'linkedin',
    query: '',
    location: '',
    pages: 1,
    dateFilter: '' as '' | (typeof DATE_FILTER_OPTIONS)[number],
    locale: 'us',
  });

  // Inline connect — avoids sending user to Settings just to link an account
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
  const [scraping, setScraping] = useState(false);
  const [scrapeJobId, setScrapeJobId] = useState<string | null>(null);
  const [scrapeOutcome, setScrapeOutcome] = useState<{ ok: boolean; note?: string } | null>(null);
  const [livePostings, setLivePostings] = useState<Posting[]>([]);
  const scrapeJobRef = useRef<string | null>(null);

  // Single event listener — handles both live job inserts and scrape lifecycle events
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
        setLivePostings((prev) => {
          if (prev.some((p) => p.id === item.id)) return prev;
          return [item, ...prev].slice(0, 500);
        });
      }
      return;
    }

    if (ev.jobId !== scrapeJobRef.current) return;

    if (ev.type === 'job.completed') {
      setScraping(false);
      setScrapeOutcome({ ok: true });
    } else if (ev.type === 'job.failed' || ev.type === 'job.cancelled') {
      setScraping(false);
      setScrapeOutcome({ ok: false, note: String(ev.data ?? 'failed') });
    }
  });

  // Merge server postings with live-streamed ones
  const allPostings = useMemo(() => {
    const seen = new Set(postings.map((p) => p.id));
    const extra = livePostings.filter((p) => !seen.has(p.id));
    return [...extra, ...postings];
  }, [postings, livePostings]);

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

    // Cancel any previously tracked job before starting — prevents concurrency errors
    // from leftover sidecar jobs the UI lost track of.
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

      // Concurrency limit hit despite cancelling the tracked job (e.g. a second
      // job was started from another session). Cancel via the sidecar and retry once.
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
    // Reset UI immediately — don't wait for a sidecar event that may never arrive.
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

  const handleClearPostings = async () => {
    await clearPostings.mutateAsync();
    setLivePostings([]);
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

    // Apply sorting
    result = [...result].sort((a, b) => {
      if (sortBy === 'newest') {
        // Sort by postedAt (newest first), fallback to capturedAt
        const aTime = a.postedAt ?? a.capturedAt;
        const bTime = b.postedAt ?? b.capturedAt;
        return bTime - aTime;
      } else if (sortBy === 'oldest') {
        // Sort by postedAt (oldest first), fallback to capturedAt
        const aTime = a.postedAt ?? a.capturedAt;
        const bTime = b.postedAt ?? b.capturedAt;
        return aTime - bTime;
      } else if (sortBy === 'company') {
        // Sort alphabetically by company name
        return a.company.localeCompare(b.company);
      }
      return 0;
    });

    return result;
  }, [allPostings, filter, sortBy]);

  return (
    <PageTransition className="flex h-full overflow-hidden">
      <div className="flex-1 overflow-y-auto px-10 py-10">
        <PageHeader
          title={t('jobs.title')}
          subtitle={t('jobs.subtitle')}
          badge={t('jobs.eyebrow')}
          actions={
            <div className="flex items-center gap-2">
              <Button
                size="sm"
                variant="glass"
                onClick={() => setShowScrapeForm(!showScrapeForm)}
                className="transition-all duration-150 ease-out"
              >
                <Plus size={12} />
                {t('jobs.scrapeJobs')}
              </Button>
              {allPostings.length > 0 && !scraping && (
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => void handleClearPostings()}
                  title={t('jobs.clearScrapedJobs')}
                >
                  <Trash2 size={12} />
                  {t('jobs.clear')}
                </Button>
              )}
              <div className="flex items-center gap-2 rounded-lg border border-white/[0.06] bg-white/[0.03] px-3 py-1.5 transition-colors focus-within:border-brand/35">
                <Search size={12} className="shrink-0 text-foreground/40" />
                <Input
                  value={filter}
                  onChange={(e) => setFilter(e.target.value)}
                  placeholder={t('jobs.searchPlaceholder')}
                  className="w-48 bg-transparent text-xs text-foreground outline-none placeholder:text-foreground/25 border-none p-0 rounded-none"
                  variant="default"
                />
              </div>
              <SelectDropdown
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
            </div>
          }
        />

        <ScrapeForm
          show={showScrapeForm}
          form={scrapeForm}
          scraping={scraping}
          scrapeOutcome={scrapeOutcome}
          boardConnected={boardConnected}
          connectPending={connectPending}
          disconnectPending={disconnectPending}
          onToggle={() => setShowScrapeForm(!showScrapeForm)}
          onFormChange={(updates) => setScrapeForm({ ...scrapeForm, ...updates })}
          onStart={startScrape}
          onCancel={cancelScrape}
          onConnect={handleInlineConnect}
          onDisconnect={handleInlineDisconnect}
          onGeocode={(q) => api.geocode.suggest(q)}
        />

        {filtered.length === 0 ? (
          <GlassCard tone="graphite" highlight className="text-center text-sm text-foreground/55">
            {t('jobs.empty')}
          </GlassCard>
        ) : (
          <div className="flex flex-col gap-2">
            {filtered.map((p, i) => (
              <motion.div
                key={p.id}
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                transition={staggeredItem(i)}
              >
                <PostingRow
                  posting={p}
                  onApply={() => setActive(p)}
                  formatRelativeTime={formatRelativeTime}
                />
              </motion.div>
            ))}
          </div>
        )}
      </div>

      <AnimatePresence>
        {active && (
          <motion.aside
            initial={{ x: 480, opacity: 0 }}
            animate={{ x: 0, opacity: 1 }}
            exit={{ x: 480, opacity: 0 }}
            transition={transition.relaxed}
            className="glass-elevated m-3 ml-0 flex w-[460px] flex-col rounded-2xl overflow-hidden"
          >
            <ApplyDrawer posting={active} onClose={() => setActive(null)} />
          </motion.aside>
        )}
      </AnimatePresence>
    </PageTransition>
  );
}
