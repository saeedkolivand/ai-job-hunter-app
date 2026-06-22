import { ListFilter, Plus, Trash2 } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';

import {
  AGGREGATOR_BOARD_ID,
  type BoardScrapeSummary,
  type DATE_FILTER_OPTIONS,
} from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, ConfirmModal, Dropdown, Input, useNotification } from '@ajh/ui';

import { PageHeader } from '@/components/layout/PageHeader';
import { PageTransition } from '@/components/layout/PageTransition';
import { JobsResults } from '@/features/jobs/components/JobsResults';
import { ScrapeForm } from '@/features/jobs/components/ScrapeForm';
import type { ScrapeFormState } from '@/features/jobs/components/ScrapeForm/constants';
import { useDefaultResumeId } from '@/features/jobs/hooks/useDefaultResumeId';
import { useScraping } from '@/features/jobs/hooks/useScraping';
import { MatchScoresProvider } from '@/features/jobs/providers';
import type { JobEvent, Posting } from '@/features/jobs/types';
import { useFormatRelativeTime } from '@/hooks/use-format-relative-time';
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
  const { filter, sortBy } = jobs;
  const setFilter = (v: string) => setJobs({ filter: v });
  const setSortBy = (v: 'newest' | 'oldest' | 'company') => setJobs({ sortBy: v });
  const [showScrapeForm, setShowScrapeForm] = useState(false);
  const [confirmClear, setConfirmClear] = useState(false);
  const [scrapeForm, setScrapeForm] = useState<ScrapeFormState>({
    boards: [AGGREGATOR_BOARD_ID],
    query: '',
    location: '',
    radiusKm: 0,
    amount: 25,
    dateFilter: '' as '' | (typeof DATE_FILTER_OPTIONS)[number],
    companies: [],
  });

  // One-way prefill: seed the scrape location from the saved preferred location
  // once it first arrives, and only if the user hasn't typed one. The ref guard
  // keeps this from re-seeding or clobbering a later user edit. Picking a location
  // here never writes back to settings.
  const { data: jobPrefs } = useJobPreferences();
  const seededLocation = useRef(false);
  useEffect(() => {
    if (seededLocation.current || !jobPrefs?.location) return;
    seededLocation.current = true;
    setScrapeForm((f) => (f.location ? f : { ...f, location: jobPrefs.location ?? '' }));
  }, [jobPrefs?.location]);

  const {
    scraping,
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
      const skippedBoards = boardSummaries.filter((b) => b.skipped === 'needs-login');
      let note: string | undefined;
      if (failedBoards.length > 0) {
        const total = boardSummaries.length;
        const done = total - failedBoards.length;
        const failedNames = failedBoards.map((b) => t(`jobs.boards.${b.board}`)).join(', ');
        note = t('jobs.partialScrapeNote', {
          done: String(done),
          total: String(total),
          failed: failedNames,
        });
      }
      // Capture the active job id BEFORE noteScrapeFinished clears scrapeJobRef.
      // Guard: only emit per-board warnings for the active scrape job — stale
      // `job.completed` events from a previous round must not show false diagnostics.
      const isActiveJob = ev.jobId === scrapeJobRef.current;
      noteScrapeFinished(ev.jobId, { ok: true, note });
      void invalidatePostings();
      if (!isActiveJob) return;
      // Surface skipped-due-to-login boards as a distinct warning notification
      // so the user knows why a board returned 0 results.
      if (skippedBoards.length > 0) {
        const boardNames = skippedBoards.map((b) => t(`jobs.boards.${b.board}`)).join(', ');
        notify.warning({
          message: t('jobs.needsLogin.skippedNote', {
            boards: boardNames,
            count: skippedBoards.length,
          }),
          // Sticky: diagnostic notification requires user action (sign in to that board).
          duration: 0,
        });
      }
      // Surface skipped-due-to-missing-company boards as a distinct warning so
      // the user knows to add a company slug in the scrape form.
      const needsCompanyBoards = boardSummaries.filter((b) => b.skipped === 'needs-company');
      if (needsCompanyBoards.length > 0) {
        const boardNames = needsCompanyBoards.map((b) => t(`jobs.boards.${b.board}`)).join(', ');
        notify.warning({
          message: t('jobs.needsCompany.skippedNote', {
            boards: boardNames,
            count: needsCompanyBoards.length,
          }),
          // Sticky: diagnostic notification requires user action (add company slug).
          duration: 0,
        });
      }
    } else if (ev.type === 'job.failed') {
      noteScrapeFinished(ev.jobId, {
        ok: false,
        note: typeof ev.data === 'string' ? ev.data : t('jobs.scrapeFailed'),
      });
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
                  variant="primary"
                  onClick={() => setShowScrapeForm(!showScrapeForm)}
                  className="transition-all duration-150 ease-out"
                >
                  <Plus size={12} />
                  {t('jobs.scrapeJobs')}
                </Button>
                {allPostings.length > 0 && !scraping && (
                  <Button
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
                  className="text-foreground/75 placeholder:text-foreground/30"
                  variant="default"
                  wrapperClassName="w-48"
                  allowClear
                />
                <Dropdown
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
            onToggle={() => setShowScrapeForm(!showScrapeForm)}
            onFormChange={(updates) => setScrapeForm({ ...scrapeForm, ...updates })}
            onStart={startScrape}
            onCancel={cancelScrape}
            onGeocode={geocodeSuggest}
          />
        </div>

        <JobsResults
          filtered={filtered}
          formatRelativeTime={formatRelativeTime}
          scraping={scraping}
          onShowMore={handleShowMore}
          onScrape={() => setShowScrapeForm(true)}
        />
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
