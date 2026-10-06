import { Pause, Pencil, Play, Trash2 } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useMemo, useRef, useState } from 'react';

import type { Autopilot, AutopilotFoundJob } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { type ActionMenuItem, ConfirmModal, GlassCard, transition, useNotification } from '@ajh/ui';

import type { AutopilotRunState } from '@/lib/machines/autopilot-run.machine';
import { useInteractions, useMarkNotDuplicate, useOpenExternal, usePersistJob } from '@/services';

import { AutopilotCardHeader } from './AutopilotCardHeader';
import { type FoundJobsSortBy, scoreVariant, sortFoundJobsByDate } from './found-jobs';
import { FoundJobsPanel } from './FoundJobsPanel';
import { useFoundJobsFocus } from './useFoundJobsFocus';

export {
  type FoundJobsSortBy,
  SCORE_VARIANTS,
  type ScoreVariant,
  sortFoundJobsByDate,
} from './found-jobs';

interface StepLog {
  step: string;
  detail: string;
  ts: number;
}

interface AutopilotCardProps {
  autopilot: Autopilot;
  runState: AutopilotRunState;
  stepLogs: StepLog[];
  /** When true (tray/deep-link focus), auto-expand found-jobs + scroll into view. */
  focused?: boolean;
  /** A specific found-job url to scroll+highlight once expanded (e.g. returning
   *  from an Apply via Back). Only meaningful when `focused` is true — falls
   *  back to centering the header when null. */
  focusedJobUrl?: string | null;
  /** Called once the focus has been consumed, so the page can clear it. */
  onFocusHandled?: () => void;
  onRun(): void;
  onTogglePause(): void;
  onEdit(): void;
  onDelete(): void;
  /** Open the dedicated apply page for a found job (#51). */
  onApply(job: AutopilotFoundJob): void;
}

const STEP_ICON: Record<string, string> = {
  scrape_start: '⟳',
  scrape_done: '✓',
  scrape_diag: '⚠',
  rerank_start: '◇',
  rerank_timeout: '◷',
  rank_done: '★',
  cancelled: '⊘',
  complete: '✓',
};

export function AutopilotCard({
  autopilot: ap,
  runState,
  stepLogs,
  focused,
  focusedJobUrl,
  onFocusHandled,
  onRun,
  onTogglePause,
  onEdit,
  onDelete,
  onApply,
}: AutopilotCardProps) {
  const paused = ap.status === 'paused';
  const running = runState === 'scraping' || runState === 'ranking';
  const { t } = useTranslation();
  const openExternal = useOpenExternal();
  const persistJob = usePersistJob();
  const split = useMarkNotDuplicate();
  const notify = useNotification();
  // View-side sort choice for THIS card's found-jobs list — local, per-card
  // state (owner correction: two expanded autopilots must be sortable
  // independently), not a session-store field. Resets on unmount, which is
  // acceptable — it's a display preference, not data. `'relevance'` (default)
  // is the STORED rank order, unchanged from today until the user opts in.
  const [sortBy, setSortBy] = useState<FoundJobsSortBy>('relevance');
  const [showFound, setShowFound] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const headerRef = useRef<HTMLDivElement>(null);
  const listContainerRef = useRef<HTMLDivElement>(null);
  // Cross-board clustering (ADR-029): render one row per cluster — the canonical
  // member. Non-canonical members (clusterCanonical === false) collapse into it;
  // unclustered/legacy rows always show. Every "Found · N" count below reads off
  // this list, so the counts track visible clusters, not raw postings.
  const foundJobs = useMemo(() => {
    const canonical = (ap.foundJobs ?? []).filter((j) => j.clusterCanonical !== false);
    return sortBy === 'relevance' ? canonical : sortFoundJobsByDate(canonical, sortBy);
  }, [ap.foundJobs, sortBy]);
  // Does this list hold BOTH scales at once? After a semantic re-rank it can:
  // the re-ranked head carries the combined "Match %", the tail keyword
  // coverage, and the backend sorts them as two separate blocks — so a 58 can
  // legitimately sit above a 62. Until now the only visible difference was the
  // tier colour (screen-reader users always had the sr-only metric name), which
  // reads as a sorting bug. When the list mixes, each row names its metric;
  // when it doesn't — the overwhelmingly common case — nothing is added, since
  // a label repeated identically on every row is noise.
  const mixedScoreSources = useMemo(
    () => new Set(foundJobs.filter((j) => typeof j.score === 'number').map(scoreVariant)).size > 1,
    [foundJobs]
  );
  const { highlightedUrl, resolvePendingScroll } = useFoundJobsFocus({
    focused,
    focusedJobUrl,
    onFocusHandled,
    headerRef,
    listContainerRef,
    setShowFound,
  });

  // Build viewed-url sets from persisted interactions (viewed + opened).
  const { data: viewedData } = useInteractions('viewed');
  const { data: openedData } = useInteractions('opened');
  const viewedUrls = useMemo(
    () =>
      new Set([
        ...(viewedData ?? []).map((r: { url?: string }) => r.url ?? ''),
        ...(openedData ?? []).map((r: { url?: string }) => r.url ?? ''),
      ]),
    [viewedData, openedData]
  );

  // #46 — secondary controls collapse into a 3-dots overflow menu; Run stays a
  // primary button. Edit is locked while a run is in flight.
  const actionItems: ActionMenuItem[] = [
    {
      label: paused ? t('autopilot.resume') : t('autopilot.pause'),
      icon: paused ? <Play size={14} /> : <Pause size={14} />,
      onSelect: onTogglePause,
    },
    {
      label: t('autopilot.edit'),
      icon: <Pencil size={14} />,
      onSelect: onEdit,
      disabled: running,
    },
    {
      label: t('autopilot.delete'),
      icon: <Trash2 size={14} />,
      onSelect: () => setConfirmDelete(true),
      destructive: true,
    },
  ];

  const handleJobClick = async (job: AutopilotFoundJob) => {
    void openExternal.mutate(job.url);
    // Also persist 'viewed' so the badge appears immediately and survives reload.
    try {
      await persistJob.mutateAsync({
        job: {
          // `job.url` doubles as the interaction's identity key — omitting it
          // collapses EVERY autopilot found job onto the single `("", "viewed")`
          // slot in InteractionStore::upsert (job_id defaults to "" server-side),
          // so only the most-recently-opened job ever showed the Viewed badge.
          id: job.url,
          url: job.url,
          title: job.title,
          company: job.company ?? '',
          location: job.location ?? '',
          source: 'autopilot',
          externalId: job.url,
          description: '',
          capturedAt: Date.now(),
        },
        interactionType: 'viewed',
      });
    } catch {
      // non-fatal: badge already shows optimistically via viewedUrls query refetch
    }
  };

  // Split this canonical job out of its cluster (ADR-029 §h): tombstone the
  // canonical member against every other member. `autopilotId` scopes the
  // recompute to this record. Success surfaced only after the mutation resolves.
  const handleSplitCluster = (job: AutopilotFoundJob) => {
    const members = job.clusterMembers ?? [];
    const canonicalKey = job.clusterId;
    if (!canonicalKey || members.length < 2) return;
    const otherKeys = members.filter((m) => m.key !== canonicalKey).map((m) => m.key);
    if (otherKeys.length === 0) return;
    split.mutate(
      { memberKey: canonicalKey, otherKeys, autopilotId: ap._id },
      {
        onSuccess: () => notify.success({ message: t('jobs.cluster.splitDone') }),
        onError: () => notify.error({ message: t('jobs.cluster.splitFailed') }),
      }
    );
  };

  return (
    <GlassCard className="flex flex-col gap-3">
      <AutopilotCardHeader
        autopilot={ap}
        runState={runState}
        running={running}
        foundCount={foundJobs.length}
        showFound={showFound}
        actionItems={actionItems}
        headerRef={headerRef}
        onRun={onRun}
        onToggleFound={() => setShowFound((v) => !v)}
      />

      {/* Live step log — only visible while running */}
      <AnimatePresence>
        {running && stepLogs.length > 0 && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: 'auto' }}
            exit={{ opacity: 0, height: 0 }}
            transition={transition.normal}
            className="overflow-hidden"
          >
            <div className="rounded-lg bg-card border border-[var(--border-clear)] px-3 py-2 space-y-1 max-h-32 overflow-y-auto">
              {stepLogs.map((log, i) => (
                <div key={i} className="flex items-start gap-2 text-[10px] leading-relaxed">
                  <span className="text-brand-soft/70 shrink-0 w-3 text-center">
                    {STEP_ICON[log.step] ?? '·'}
                  </span>
                  <span className="text-foreground/50 font-mono">{log.detail}</span>
                </div>
              ))}
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Found jobs from the most recent run */}
      <AnimatePresence>
        {showFound && foundJobs.length > 0 && (
          <FoundJobsPanel
            jobs={foundJobs}
            sortBy={sortBy}
            onSortChange={setSortBy}
            onCollapse={() => setShowFound(false)}
            onAnimationComplete={resolvePendingScroll}
            listContainerRef={listContainerRef}
            highlightedUrl={highlightedUrl}
            mixedScoreSources={mixedScoreSources}
            viewedUrls={viewedUrls}
            splitPending={split.isPending}
            onOpen={(job) => void handleJobClick(job)}
            onApply={onApply}
            onSplitCluster={handleSplitCluster}
          />
        )}
      </AnimatePresence>

      <ConfirmModal
        open={confirmDelete}
        onClose={() => setConfirmDelete(false)}
        onConfirm={() => {
          setConfirmDelete(false);
          onDelete();
        }}
        title={t('autopilot.deleteTitle')}
        description={t('autopilot.deleteDescription')}
        confirmText={t('autopilot.delete')}
        variant="danger"
      />
    </GlassCard>
  );
}
