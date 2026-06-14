import { ListFilter, Plus, Search, Trash2 } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';

import type { DATE_FILTER_OPTIONS } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import {
  Button,
  ConfirmModal,
  Dropdown,
  EmptyState,
  GlassCard,
  Input,
  useNotification,
} from '@ajh/ui';

import { PageHeader } from '@/components/layout/PageHeader';
import { PageTransition } from '@/components/layout/PageTransition';
import { PostingRow } from '@/features/jobs/components/PostingRow';
import { ScrapeForm } from '@/features/jobs/components/ScrapeForm';
import type { ScrapeFormState } from '@/features/jobs/components/ScrapeForm/constants';
import { useDefaultResumeId } from '@/features/jobs/hooks/useDefaultResumeId';
import { useFormatRelativeTime } from '@/features/jobs/hooks/useFormatRelativeTime';
import { useScraping } from '@/features/jobs/hooks/useScraping';
import { MatchScoresProvider } from '@/features/jobs/providers';
import type { JobEvent, Posting } from '@/features/jobs/types';
import { useClearPostings, useGeocodeSuggest, useJobEvents, usePostings } from '@/services';
import { useSessionStore } from '@/store/session-store';

export function JobsPage() {
  const { t } = useTranslation();

  const formatRelativeTime = useFormatRelativeTime(t);

  const geocodeSuggest = useGeocodeSuggest();
  const notify = useNotification();
  const { data: postingsData = [] } = usePostings();
  const postings = postingsData as Posting[];
  const clearPostings = useClearPostings();

  const { jobs, setJobs } = useSessionStore();
  const { filter, sortBy } = jobs;
  const setFilter = (v: string) => setJobs({ filter: v });
  const setSortBy = (v: 'newest' | 'oldest' | 'company') => setJobs({ sortBy: v });
  const [showScrapeForm, setShowScrapeForm] = useState(false);
  const [confirmClear, setConfirmClear] = useState(false);
  const [scrapeForm, setScrapeForm] = useState<ScrapeFormState>({
    board: 'linkedin',
    query: '',
    location: '',
    radiusKm: 0,
    amount: 25,
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
    noteScrapeFinished,
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

    if (ev.type === 'job.completed') {
      noteScrapeFinished(ev.jobId, { ok: true });
    } else if (ev.type === 'job.failed') {
      noteScrapeFinished(ev.jobId, {
        ok: false,
        note: typeof ev.data === 'string' ? ev.data : t('jobs.scrapeFailed'),
      });
    }
  });

  useEffect(() => {
    if (livePostings.length > 0) setShowScrapeForm(false);
  }, [livePostings.length]);

  const allPostings = useMemo(() => {
    const seen = new Set(postings.map((p) => p.id));
    const extra = livePostings.filter((p) => !seen.has(p.id));
    return [...extra, ...postings];
  }, [postings, livePostings]);

  const handleClearPostings = async () => {
    setConfirmClear(false);
    await clearPostings.mutateAsync();
    setLivePostings([]);
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
  const jobIds = useMemo(() => filtered.map((p) => p.id), [filtered]);

  // Windowed list: only the visible rows (plus a small overscan) are mounted, so
  // a long postings list doesn't paint hundreds of glass rows at once. Keyed by
  // posting id so measurement survives live-prepended rows during a scrape.
  const scrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: filtered.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 88,
    overscan: 6,
    getItemKey: (index) => filtered[index]?.id ?? index,
  });

  return (
    <MatchScoresProvider resumeId={resumeId} jobIds={jobIds}>
      <PageTransition className="flex h-full flex-col overflow-hidden">
        {/* Pinned header + scrape form; the list below owns the scroll. */}
        <div className="shrink-0 px-10 pt-10">
          <PageHeader
            title={t('jobs.title')}
            subtitle={t('jobs.subtitle')}
            badge={t('jobs.eyebrow')}
            actions={
              <div className="flex items-center gap-2">
                <Button
                  size="sm"
                  variant="primary"
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
                    onClick={() => setConfirmClear(true)}
                    title={t('jobs.clearScrapedJobs')}
                  >
                    <Trash2 size={12} />
                    {t('jobs.clear')}
                  </Button>
                )}
                <Input
                  prefix={<ListFilter size={12} />}
                  value={filter}
                  onChange={(e) => setFilter(e.target.value)}
                  placeholder={t('jobs.searchPlaceholder')}
                  className="w-48 text-xs text-foreground/75 placeholder:text-foreground/30"
                  variant="default"
                  wrapperClassName="h-7"
                  allowClear
                />
                <Dropdown
                  size="sm"
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
            onGeocode={geocodeSuggest}
          />
        </div>

        <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto px-10 pb-10">
          {filtered.length === 0 ? (
            <GlassCard>
              <EmptyState
                icon={Search}
                title={t('jobs.empty')}
                action={
                  <Button variant="primary" size="sm" onClick={() => setShowScrapeForm(true)}>
                    <Search size={13} /> {t('jobs.emptyCta')}
                  </Button>
                }
                className="py-10"
              />
            </GlassCard>
          ) : (
            <div
              style={{ height: virtualizer.getTotalSize(), position: 'relative', width: '100%' }}
            >
              {virtualizer.getVirtualItems().map((vi) => {
                const posting = filtered[vi.index];
                if (!posting) return null;
                return (
                  <div
                    key={vi.key}
                    data-index={vi.index}
                    ref={virtualizer.measureElement}
                    style={{
                      position: 'absolute',
                      top: 0,
                      left: 0,
                      width: '100%',
                      transform: `translateY(${vi.start}px)`,
                    }}
                  >
                    {/* pb-2 reproduces the old gap-2 between rows (included in the
                      measured height so virtual offsets stay correct). */}
                    <div className="pb-2">
                      <PostingRow posting={posting} formatRelativeTime={formatRelativeTime} />
                    </div>
                  </div>
                );
              })}
            </div>
          )}

          {allPostings.length > 0 && (
            <div className="flex justify-center pt-4">
              <Button variant="ghost" size="sm" onClick={handleShowMore} loading={scraping}>
                {!scraping && <Plus size={12} />}
                {t('jobs.showMore')}
              </Button>
            </div>
          )}
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
