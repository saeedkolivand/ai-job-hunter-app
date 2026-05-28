import { Plus, Search, Trash2 } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useMemo, useState } from 'react';

import type { DATE_FILTER_OPTIONS } from '@ajh/shared';
import {
  Button,
  GlassCard,
  Input,
  SelectDropdown,
  staggeredItem,
  transition,
  useNotification,
} from '@ajh/ui';

import { PageHeader } from '@/components/layout/PageHeader';
import { PageTransition } from '@/components/layout/PageTransition';
import { ApplyDrawer } from '@/features/jobs/components/ApplyDrawer';
import { PostingRow } from '@/features/jobs/components/PostingRow';
import { ScrapeForm } from '@/features/jobs/components/ScrapeForm';
import { useFormatRelativeTime } from '@/features/jobs/hooks/useFormatRelativeTime';
import { useScraping } from '@/features/jobs/hooks/useScraping';
import type { JobEvent, Posting } from '@/features/jobs/types';
import { useTranslation } from '@/lib/i18n';
import { useAppClient } from '@/providers/AppClientProvider';
import { useClearPostings, useJobEvents, usePostings } from '@/services';
import { useSessionStore } from '@/store/session-store';

export function JobsPage() {
  const { t } = useTranslation();

  const formatRelativeTime = useFormatRelativeTime(t);

  const api = useAppClient();
  const notify = useNotification();
  const { data: postingsData = [] } = usePostings();
  const postings = postingsData as Posting[];
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

  const {
    scraping,
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
  } = useScraping(notify, scrapeForm);

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
  });

  const allPostings = useMemo(() => {
    const seen = new Set(postings.map((p) => p.id));
    const extra = livePostings.filter((p) => !seen.has(p.id));
    return [...extra, ...postings];
  }, [postings, livePostings]);

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
