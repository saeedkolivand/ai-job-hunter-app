import {
  AlertCircle,
  Bookmark,
  Building2,
  CheckCircle2,
  CircleCheck,
  Copy,
  ExternalLink,
  Eye,
  Info,
  Loader2,
  MapPin,
  Plus,
  Search,
  Send,
  ShieldAlert,
  Trash2,
  X,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useMemo, useRef, useState } from 'react';
import { createFileRoute } from '@tanstack/react-router';

import type { DATE_FILTER_OPTIONS, JobInteraction } from '@ajh/shared';
import {
  Button,
  GlassCard,
  Input,
  LocationInput,
  SelectDropdown,
  SourceBadge,
  TextArea,
  useNotification,
} from '@ajh/ui';

import { PageHeader } from '@/components/layout/PageHeader';
import { PageTransition } from '@/components/layout/PageTransition';
import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import { staggeredItem, transition } from '@/lib/motion';
import { useAppClient } from '@/providers/AppClientProvider';
import {
  useApplyJob,
  useBoardConnect,
  useBoardDisconnect,
  useBoardStatus,
  useCancelJob,
  useClearPostings,
  useJobEvents,
  useLinkedInConnect,
  useLinkedInDisconnect,
  useLinkedInStatus,
  useOpenExternal,
  usePersistJob,
  usePostings,
  useScrapeBoard,
} from '@/services';

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

interface StepEvent {
  kind: 'step';
  stage: string;
  ok: boolean;
  note?: string;
}
interface ProgressEvent {
  kind: 'progress';
  stage: string;
  p: number;
}

const APPLIABLE = new Set(['linkedin', 'indeed', 'greenhouse', 'workday', 'xing', 'glassdoor']);
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

  const [filter, setFilter] = useState('');
  const [sortBy, setSortBy] = useState<'newest' | 'oldest' | 'company'>('newest');
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

        <AnimatePresence>
          {showScrapeForm && (
            <motion.div
              initial={{ opacity: 0, y: -8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -8 }}
              transition={transition.normal}
              className="mb-4"
            >
              <GlassCard tone="graphite" highlight className="p-5">
                {/* Header */}
                <div className="mb-4 flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <div className="flex h-5 w-5 items-center justify-center rounded-md bg-brand/15">
                      <Search size={11} className="text-brand-soft" />
                    </div>
                    <span className="text-xs font-medium text-foreground/70">
                      {t('jobs.newScrape')}
                    </span>
                  </div>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setShowScrapeForm(false)}
                    className="rounded-md p-1 text-foreground/40 hover:bg-white/5 hover:text-foreground/70 h-auto"
                  >
                    <X size={13} />
                  </Button>
                </div>

                {/* Query — hero input */}
                <div className="mb-4">
                  <Input
                    type="text"
                    value={scrapeForm.query}
                    onChange={(e) => setScrapeForm({ ...scrapeForm, query: e.target.value })}
                    placeholder={t('jobs.queryPlaceholder')}
                    disabled={scraping}
                    className="w-full bg-white/[0.03] text-sm text-foreground placeholder:text-foreground/25 disabled:opacity-50"
                  />
                </div>

                {/* Board picker */}
                <div className="mb-4">
                  <div className="mb-2 text-[10px] font-medium uppercase tracking-[0.18em] text-foreground/35">
                    {t('jobs.board')}
                  </div>
                  <div className="flex flex-wrap gap-1.5">
                    {(
                      [
                        { id: 'linkedin', label: t('jobs.boards.linkedin') },
                        { id: 'indeed', label: t('jobs.boards.indeed') },
                        { id: 'stepstone', label: t('jobs.boards.stepstone') },
                        { id: 'xing', label: t('jobs.boards.xing') },
                        { id: 'arbeitsagentur', label: t('jobs.boards.arbeitsagentur') },
                        { id: 'berlinstartupjobs', label: t('jobs.boards.berlinstartupjobs') },
                        { id: 'germantechjobs', label: t('jobs.boards.germantechjobs') },
                        { id: 'greenhouse', label: t('jobs.boards.greenhouse') },
                        { id: 'lever', label: t('jobs.boards.lever') },
                        { id: 'ashby', label: t('jobs.boards.ashby') },
                        { id: 'workday', label: t('jobs.boards.workday') },
                        { id: 'smartrecruiters', label: t('jobs.boards.smartrecruiters') },
                        { id: 'recruitee', label: t('jobs.boards.recruitee') },
                        { id: 'personio', label: t('jobs.boards.personio') },
                        { id: 'remoteok', label: t('jobs.boards.remoteok') },
                        { id: 'remotive', label: t('jobs.boards.remotive') },
                        { id: 'arbeitnow', label: t('jobs.boards.arbeitnow') },
                        { id: 'wwr', label: t('jobs.boards.wwr') },
                        { id: 'ycombinator', label: t('jobs.boards.ycombinator') },
                      ] as const
                    ).map(({ id, label }) => {
                      const active = scrapeForm.board === id;
                      return (
                        <Button
                          key={id}
                          variant="ghost"
                          disabled={scraping}
                          onClick={() => setScrapeForm({ ...scrapeForm, board: id })}
                          className={cn(
                            'rounded-lg px-2.5 py-1 text-[11px] font-medium transition-all',
                            active
                              ? 'bg-brand/20 text-brand-soft ring-1 ring-brand/40'
                              : 'bg-white/[0.04] text-foreground/50 hover:bg-white/[0.07] hover:text-foreground/80',
                            'disabled:cursor-not-allowed disabled:opacity-40'
                          )}
                        >
                          {label}
                        </Button>
                      );
                    })}
                  </div>
                </div>

                {/* Auth mode badge — shown for boards that support authentication */}
                <AnimatePresence>
                  {AUTH_BENEFITS.has(scrapeForm.board) && (
                    <motion.div
                      key={`mode-${scrapeForm.board}-${boardConnected ? 'auth' : 'guest'}`}
                      initial={{ opacity: 0 }}
                      animate={{ opacity: 1 }}
                      exit={{ opacity: 0 }}
                      transition={transition.fast}
                      className="mb-3 flex items-center gap-1.5"
                    >
                      {boardConnected ? (
                        <>
                          <span className="inline-flex items-center gap-1 rounded-full bg-emerald-500/15 px-2 py-0.5 text-[10px] font-medium text-emerald-400">
                            <span className="h-1.5 w-1.5 rounded-full bg-emerald-400" />
                            {t('jobs.modeAuthenticated')}
                          </span>
                          <span className="text-[10px] text-foreground/35">
                            {t('jobs.modeAuthNote')}
                          </span>
                          <button
                            type="button"
                            disabled={disconnectPending}
                            onClick={() => void handleInlineDisconnect()}
                            className="ml-auto shrink-0 text-[10px] text-red-400/70 underline-offset-2 hover:text-red-400 hover:underline disabled:opacity-50"
                          >
                            {disconnectPending ? (
                              <Loader2 size={10} className="animate-spin" />
                            ) : (
                              t('jobs.disconnect')
                            )}
                          </button>
                        </>
                      ) : (
                        <>
                          <span className="inline-flex items-center gap-1 rounded-full bg-amber-500/15 px-2 py-0.5 text-[10px] font-medium text-amber-400">
                            <span className="h-1.5 w-1.5 rounded-full bg-amber-400" />
                            {t('jobs.modeGuest')}
                          </span>
                          <span className="text-[10px] text-foreground/35">
                            {t('jobs.modeGuestNote')}
                          </span>
                        </>
                      )}
                    </motion.div>
                  )}
                </AnimatePresence>

                {/* Auth hint — shown for boards that benefit from a connected account */}
                <AnimatePresence>
                  {AUTH_BENEFITS.has(scrapeForm.board) && !boardConnected && (
                    <motion.div
                      key="auth-hint"
                      initial={{ opacity: 0, height: 0, marginBottom: 0 }}
                      animate={{ opacity: 1, height: 'auto', marginBottom: 16 }}
                      exit={{ opacity: 0, height: 0, marginBottom: 0 }}
                      transition={transition.fast}
                      className="overflow-hidden"
                    >
                      <div className="flex items-center gap-2 rounded-lg border border-blue-400/15 bg-blue-400/5 px-3 py-2 text-[11px] text-blue-200/75">
                        <Info size={12} className="shrink-0 text-blue-400/60" />
                        <span>{t('jobs.authHint')}</span>
                        <button
                          type="button"
                          disabled={connectPending}
                          onClick={() => void handleInlineConnect()}
                          className="ml-auto shrink-0 text-brand-soft underline-offset-2 hover:underline disabled:opacity-50"
                        >
                          {connectPending ? (
                            <Loader2 size={11} className="animate-spin" />
                          ) : (
                            t('jobs.authHintLink')
                          )}
                        </button>
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>

                {/* Filters row */}
                <div className="mb-4 grid grid-cols-4 gap-2">
                  <div className="col-span-2">
                    <label className="mb-1.5 block text-[10px] font-medium uppercase tracking-[0.18em] text-foreground/35">
                      {t('jobs.location')}
                    </label>
                    <LocationInput
                      value={scrapeForm.location}
                      onChange={(v) => setScrapeForm({ ...scrapeForm, location: v })}
                      placeholder={t('jobs.locationPlaceholder')}
                      disabled={scraping}
                      onFetchSuggestions={(q) => api.geocode.suggest(q)}
                    />
                  </div>
                  <div>
                    <label className="mb-1.5 block text-[10px] font-medium uppercase tracking-[0.18em] text-foreground/35">
                      {t('jobs.posted')}
                    </label>
                    <SelectDropdown
                      options={[
                        { value: '', label: t('jobs.anyTime') },
                        ...(AUTH_BENEFITS.has(scrapeForm.board) && boardConnected
                          ? [
                              { value: '30m', label: t('jobs.past30m') },
                              { value: '1h', label: t('jobs.past1h') },
                              { value: '2h', label: t('jobs.past2h') },
                              { value: '4h', label: t('jobs.past4h') },
                              { value: '8h', label: t('jobs.past8h') },
                            ]
                          : []),
                        { value: '24h', label: t('jobs.past24h') },
                        { value: 'week', label: t('jobs.pastWeek') },
                        { value: 'month', label: t('jobs.pastMonth') },
                      ]}
                      value={scrapeForm.dateFilter}
                      onChange={(value) =>
                        setScrapeForm({
                          ...scrapeForm,
                          dateFilter: value as '' | (typeof DATE_FILTER_OPTIONS)[number],
                        })
                      }
                      disabled={scraping}
                      placeholder={t('jobs.anyTime')}
                    />
                  </div>
                  <div>
                    <label className="mb-1.5 block text-[10px] font-medium uppercase tracking-[0.18em] text-foreground/35">
                      {t('jobs.pages')}
                    </label>
                    <Input
                      type="number"
                      min="1"
                      max="20"
                      value={scrapeForm.pages}
                      onChange={(e) =>
                        setScrapeForm({ ...scrapeForm, pages: parseInt(e.target.value) || 1 })
                      }
                      disabled={scraping}
                      className="w-full bg-white/[0.03] text-xs text-foreground disabled:opacity-50"
                    />
                  </div>

                  {/* Indeed region — spans full width, only shown when needed */}
                  {scrapeForm.board === 'indeed' && (
                    <div className="col-span-4">
                      <label className="mb-1.5 block text-[10px] font-medium uppercase tracking-[0.18em] text-foreground/35">
                        {t('jobs.region')}
                      </label>
                      <SelectDropdown
                        options={[
                          { value: 'us', label: t('jobs.regions.us') },
                          { value: 'de', label: t('jobs.regions.de') },
                          { value: 'uk', label: t('jobs.regions.uk') },
                          { value: 'fr', label: t('jobs.regions.fr') },
                          { value: 'at', label: t('jobs.regions.at') },
                          { value: 'ch', label: t('jobs.regions.ch') },
                          { value: 'au', label: t('jobs.regions.au') },
                          { value: 'ca', label: t('jobs.regions.ca') },
                          { value: 'nl', label: t('jobs.regions.nl') },
                          { value: 'be', label: t('jobs.regions.be') },
                          { value: 'es', label: t('jobs.regions.es') },
                          { value: 'it', label: t('jobs.regions.it') },
                          { value: 'pl', label: t('jobs.regions.pl') },
                          { value: 'br', label: t('jobs.regions.br') },
                          { value: 'in', label: t('jobs.regions.in') },
                          { value: 'sg', label: t('jobs.regions.sg') },
                          { value: 'jp', label: t('jobs.regions.jp') },
                        ]}
                        value={scrapeForm.locale}
                        onChange={(value) => setScrapeForm({ ...scrapeForm, locale: value })}
                        disabled={scraping}
                        placeholder={t('jobs.selectRegion')}
                      />
                    </div>
                  )}
                </div>

                {/* Progress bar — only shown while scraping */}
                {scraping && (
                  <div className="mb-4">
                    <div className="h-px w-full overflow-hidden rounded-full bg-white/[0.06]">
                      <motion.div
                        className="h-full rounded-full bg-gradient-to-r from-brand to-primary"
                        initial={{ width: '0%' }}
                        animate={{ width: '85%' }}
                        transition={transition.fakeProgress}
                      />
                    </div>
                    <div className="mt-1.5 flex items-center gap-1.5 text-[11px] text-foreground/40">
                      <Loader2 size={10} className="animate-spin" />
                      {t('jobs.scraping')} {scrapeForm.board}…
                    </div>
                  </div>
                )}

                {/* Footer */}
                <div className="flex items-center justify-end gap-2">
                  {scraping ? (
                    <Button size="sm" variant="ghost" onClick={() => void cancelScrape()}>
                      {t('jobs.cancel')}
                    </Button>
                  ) : (
                    scrapeOutcome && (
                      <span
                        className={cn(
                          'text-[11px]',
                          scrapeOutcome.ok ? 'text-emerald-400/70' : 'text-amber-400/70'
                        )}
                      >
                        {scrapeOutcome.ok
                          ? t('jobs.done')
                          : (scrapeOutcome.note ?? t('jobs.failed'))}
                      </span>
                    )
                  )}
                  <Button
                    size="sm"
                    variant="glass"
                    onClick={() => void startScrape()}
                    disabled={scraping || !scrapeForm.query.trim()}
                    loading={scraping}
                    className="transition-all duration-150 ease-out"
                  >
                    {!scraping && <Search size={12} />}
                    {scraping ? t('jobs.scraping') : t('jobs.startScrape')}
                  </Button>
                </div>
              </GlassCard>
            </motion.div>
          )}
        </AnimatePresence>

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

function PostingRow({
  posting,
  onApply,
  formatRelativeTime,
}: {
  posting: Posting;
  onApply: () => void;
  formatRelativeTime: (timestamp?: number) => string;
}) {
  const { t } = useTranslation();
  const notify = useNotification();
  const canApply = APPLIABLE.has(posting.source);
  const openExternalMutation = useOpenExternal();
  const persistJobMutation = usePersistJob();

  const [interactionTypes, setInteractionTypes] = useState(
    () => new Set(posting.interactions?.map((i) => i.interactionType) || [])
  );

  const jobPayload = {
    id: posting.id,
    source: posting.source,
    externalId: posting.externalId,
    url: posting.url,
    title: posting.title,
    company: posting.company,
    location: posting.location,
    description: posting.description,
    capturedAt: posting.capturedAt,
  };

  const trackInteraction = async (
    interactionType: 'viewed' | 'opened' | 'applied' | 'bookmarked'
  ) => {
    setInteractionTypes((prev) => new Set([...prev, interactionType]));
    try {
      await persistJobMutation.mutateAsync({ job: jobPayload, interactionType });
    } catch (err) {
      console.error('Failed to track interaction:', err);
    }
  };

  const handleOpen = () => {
    void trackInteraction('opened');
    void openExternalMutation.mutateAsync(posting.url);
  };

  const handleCopyLink = async () => {
    try {
      await navigator.clipboard.writeText(posting.url);
      notify(t('jobs.copyLink'), 'success');
    } catch {
      notify('Failed to copy link', 'error');
    }
  };

  const handleApply = () => {
    void trackInteraction('applied');
    onApply();
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={transition.normal}
      className="relative group"
    >
      <div className="glass-graphite glass-highlight relative flex items-center gap-5 rounded-xl p-4 pl-5 transition-all duration-300 hover:bg-white/[0.03] hover:shadow-lg hover:shadow-brand/5 overflow-hidden">
        {/* Subtle ambient glow for whole card */}
        <div
          className="pointer-events-none absolute inset-0 opacity-0 group-hover:opacity-100 transition-opacity duration-300 ease-out"
          style={{
            background:
              'linear-gradient(135deg, rgba(168,85,247,0.12) 0%, rgba(99,102,241,0.08) 50%, rgba(168,85,247,0.12) 100%)',
            filter: 'blur-xl',
          }}
        />
        <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-white/10 to-white/5 text-[11px] uppercase tracking-wider text-brand-soft font-semibold shadow-inner">
          {posting.source.slice(0, 2)}
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 text-sm font-semibold text-foreground/95 tracking-tight">
            <span className="truncate">{posting.title}</span>
            {posting.remote && (
              <span className="rounded-full border border-emerald-400/20 bg-emerald-400/5 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-emerald-200/85">
                {t('jobs.remote')}
              </span>
            )}
            {/* Interaction indicators */}
            {interactionTypes.has('applied') && (
              <span className="rounded-full border border-purple-400/20 bg-purple-400/5 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-purple-200/85 flex items-center gap-1">
                <CircleCheck size={8} /> {t('jobs.applied')}
              </span>
            )}
            {interactionTypes.has('opened') && (
              <span className="rounded-full border border-blue-400/20 bg-blue-400/5 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-blue-200/85 flex items-center gap-1">
                <Eye size={8} /> {t('jobs.viewed')}
              </span>
            )}
            {interactionTypes.has('bookmarked') && (
              <span className="rounded-full border border-amber-400/20 bg-amber-400/5 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-amber-200/85 flex items-center gap-1">
                <Bookmark size={8} /> {t('jobs.saved')}
              </span>
            )}
          </div>
          <div className="mt-1 flex items-center gap-4 text-[11px]">
            <span className="flex items-center gap-1.5 text-foreground/85">
              <Building2 size={10} /> {posting.company}
            </span>
            {posting.location && (
              <span className="flex items-center gap-1.5 text-foreground/60">
                <MapPin size={10} /> {posting.location}
              </span>
            )}
            <SourceBadge source={posting.source} url={posting.url} />
            {posting.postedAt && (
              <span className="text-foreground/40">· {formatRelativeTime(posting.postedAt)}</span>
            )}
          </div>
        </div>
        <div className="flex items-center gap-2">
          <a
            href={posting.url}
            onClick={(e) => {
              e.preventDefault();
              void handleOpen();
            }}
            className="flex items-center gap-1.5 rounded-lg bg-white/5 px-2.5 py-1.5 text-[11px] text-foreground/70 hover:text-foreground hover:bg-white/10 transition-all duration-200"
          >
            <ExternalLink size={10} /> {t('jobs.open')}
          </a>
          <button
            onClick={handleCopyLink}
            className="flex items-center gap-1.5 rounded-lg bg-white/5 px-2.5 py-1.5 text-[11px] text-foreground/70 hover:text-foreground hover:bg-white/10 transition-all duration-200"
            title={t('jobs.copyLink')}
          >
            <Copy size={10} />
          </button>
          <Button
            size="sm"
            variant={canApply ? 'glass' : 'ghost'}
            onClick={handleApply}
            disabled={!canApply}
            title={canApply ? '' : t('jobs.applyNotSupported')}
            className={cn(
              'transition-all duration-150 ease-out',
              canApply ? '' : 'cursor-not-allowed'
            )}
          >
            <Send size={11} /> {t('jobs.apply')}
          </Button>
        </div>
      </div>
    </motion.div>
  );
}

interface ApplyStep {
  ts: number;
  stage: string;
  ok: boolean;
  note?: string;
  kind: 'step' | 'progress';
  p?: number;
}

function ApplyDrawer({ posting, onClose }: { posting: Posting; onClose: () => void }) {
  const { t } = useTranslation();
  const [autoSubmit, setAutoSubmit] = useState(false);
  const [coverLetter, setCoverLetter] = useState('');
  const [running, setRunning] = useState(false);
  const [jobId, setJobId] = useState<string | null>(null);
  const [steps, setSteps] = useState<ApplyStep[]>([]);
  const [outcome, setOutcome] = useState<{ ok: boolean; submitted: boolean; note?: string } | null>(
    null
  );
  const jobRef = useRef<string | null>(null);
  const applyJob = useApplyJob();
  const cancelJobMutation = useCancelJob();

  jobRef.current = jobId;

  useJobEvents((raw: unknown) => {
    const ev = raw as JobEvent;
    if (ev.jobId !== jobRef.current) return;
    if (ev.type === 'job.stream') {
      const data = ev.data as StepEvent | ProgressEvent;
      setSteps((prev) =>
        [
          ...prev,
          {
            ts: ev.ts,
            stage: data.stage,
            ok: 'ok' in data ? data.ok : true,
            ...('note' in data && data.note ? { note: data.note } : {}),
            kind: data.kind,
            ...(data.kind === 'progress' ? { p: data.p } : {}),
          },
        ].slice(-40)
      );
    } else if (ev.type === 'job.completed') {
      const r = ev.data as { ok: boolean; submitted: boolean; note?: string };
      setOutcome(r);
      setRunning(false);
    } else if (ev.type === 'job.failed' || ev.type === 'job.cancelled') {
      setOutcome({ ok: false, submitted: false, note: String(ev.data ?? 'failed') });
      setRunning(false);
    }
  });

  const start = async () => {
    setSteps([]);
    setOutcome(null);
    setRunning(true);
    try {
      const res = (await applyJob.mutateAsync({
        board: posting.source,
        url: posting.url,
        ...(coverLetter.trim() ? { coverLetter: coverLetter.trim() } : {}),
        autoSubmit,
      })) as { jobId: string };
      jobRef.current = res.jobId;
      setJobId(res.jobId);
    } catch (err) {
      setOutcome({
        ok: false,
        submitted: false,
        note: err instanceof Error ? err.message : String(err),
      });
      setRunning(false);
    }
  };

  const cancel = async () => {
    if (jobId) await cancelJobMutation.mutateAsync(jobId);
  };

  const canApply = APPLIABLE.has(posting.source);

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-start justify-between border-b border-white/5 p-5">
        <div className="min-w-0">
          <div className="text-[10px] uppercase tracking-[0.3em] text-foreground/40">
            {t('jobs.applyDrawer.eyebrow')}
          </div>
          <div className="mt-1 truncate text-base font-medium text-foreground">{posting.title}</div>
          <div className="mt-0.5 truncate text-xs text-foreground/55">
            {posting.company} · {posting.source}
          </div>
        </div>
        <Button
          onClick={onClose}
          className="rounded-lg bg-white/5 p-1.5 text-foreground/60 hover:text-foreground h-auto border-transparent"
          aria-label={t('jobs.close')}
        >
          <X size={14} />
        </Button>
      </header>

      {!canApply ? (
        <div className="flex-1 p-5">
          <GlassCard tone="violet" highlight className="text-sm text-foreground/70">
            {t('jobs.applyNotSupported')}
          </GlassCard>
        </div>
      ) : (
        <>
          <div className="flex-1 overflow-y-auto p-5">
            {/* ToS warning */}
            <div className="mb-4 flex items-start gap-2 rounded-xl border border-amber-400/15 bg-amber-400/5 p-3 text-xs text-amber-200/90">
              <ShieldAlert size={14} className="mt-0.5 shrink-0" />
              <div>
                <div className="font-medium">{t('jobs.applyDrawer.warningTitle')}</div>
                <div className="mt-0.5 text-[11px] text-foreground/60">
                  {t('jobs.applyDrawer.warningBody')}
                </div>
              </div>
            </div>

            {/* Cover letter input */}
            <div className="mb-5">
              <label className="mb-2 block text-xs font-medium text-foreground/90">
                {t('jobs.applyDrawer.coverLetter')}
              </label>
              <TextArea
                value={coverLetter}
                onChange={(e) => setCoverLetter(e.target.value)}
                placeholder={t('jobs.applyDrawer.coverLetterPlaceholder')}
                disabled={running}
                rows={4}
                className="w-full bg-white/[0.03] text-xs text-foreground disabled:opacity-50"
              />
            </div>

            {/* Auto-submit toggle */}
            <div className="mb-5 flex items-center gap-2.5 rounded-xl bg-white/[0.02] p-3">
              <input
                type="checkbox"
                checked={autoSubmit}
                onChange={(e) => setAutoSubmit(e.target.checked)}
                disabled={running}
                className="h-4 w-4 accent-[var(--color-brand)] cursor-pointer"
              />
              <div
                className="flex-1 cursor-pointer"
                onClick={() => !running && setAutoSubmit(!autoSubmit)}
              >
                <div className="text-xs font-medium text-foreground/90">
                  {t('jobs.applyDrawer.autoSubmit')}
                </div>
                <div className="text-[11px] text-foreground/45">
                  {t('jobs.applyDrawer.autoSubmitHint')}
                </div>
              </div>
            </div>

            {/* Step feed */}
            {steps.length > 0 && (
              <div className="mb-4">
                <div className="mb-2 text-[10px] font-medium uppercase tracking-[0.18em] text-foreground/40">
                  {t('jobs.applyDrawer.steps')}
                </div>
                <div className="space-y-1.5">
                  <AnimatePresence initial={false}>
                    {steps.map((s, i) => (
                      <StepRow key={`${s.ts}-${i}`} step={s} />
                    ))}
                  </AnimatePresence>
                </div>
              </div>
            )}

            {outcome && (
              <GlassCard
                tone={outcome.ok ? 'indigo' : 'graphite'}
                highlight
                glow={outcome.ok}
                className="mt-3"
              >
                <div className="flex items-center gap-2 text-sm">
                  {outcome.ok ? (
                    <CheckCircle2 size={14} className="text-emerald-300" />
                  ) : (
                    <AlertCircle size={14} className="text-amber-300" />
                  )}
                  <span className="font-medium">
                    {outcome.submitted
                      ? t('jobs.applyDrawer.submitted')
                      : outcome.ok
                        ? t('jobs.applyDrawer.reviewPending')
                        : t('jobs.applyDrawer.failed')}
                  </span>
                </div>
                {outcome.note && (
                  <div className="mt-1 text-[11px] text-foreground/55">{outcome.note}</div>
                )}
              </GlassCard>
            )}
          </div>

          <footer className="flex items-center justify-end gap-2 border-t border-white/5 p-4">
            {running && (
              <Button size="sm" variant="ghost" onClick={() => void cancel()}>
                {t('jobs.applyDrawer.cancel')}
              </Button>
            )}
            <Button
              size="md"
              variant={running ? 'ghost' : 'glass'}
              onClick={() => void start()}
              disabled={running}
              loading={running}
              className="transition-all duration-150 ease-out"
            >
              {!running && <Send size={14} />}
              {running ? t('jobs.applyDrawer.running') : t('jobs.applyDrawer.start')}
            </Button>
          </footer>
        </>
      )}
    </div>
  );
}

function StepRow({ step }: { step: ApplyStep }) {
  if (step.kind === 'progress') {
    return (
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        className="rounded-md bg-white/[0.02] px-2.5 py-1.5"
      >
        <div className="mb-1 flex items-center justify-between text-[11px]">
          <span className="text-foreground/65">{step.stage}</span>
          <span className="text-foreground/45">{Math.round((step.p ?? 0) * 100)}%</span>
        </div>
        <div className="h-1 overflow-hidden rounded-full bg-white/5">
          <div
            className="h-full rounded-full bg-gradient-to-r from-brand to-primary"
            style={{ width: `${(step.p ?? 0) * 100}%` }}
          />
        </div>
      </motion.div>
    );
  }
  return (
    <motion.div
      initial={{ opacity: 0, x: 6 }}
      animate={{ opacity: 1, x: 0 }}
      className="flex items-start gap-2 text-[12px]"
    >
      <span
        className={cn(
          'mt-1.5 h-1.5 w-1.5 flex-shrink-0 rounded-full shadow-[0_0_8px_currentColor]',
          step.ok ? 'bg-emerald-400' : 'bg-amber-400'
        )}
      />
      <div className="flex-1">
        <span className="text-foreground/85">{step.stage}</span>
        {step.note && <div className="text-[11px] text-foreground/45">{step.note}</div>}
      </div>
    </motion.div>
  );
}
