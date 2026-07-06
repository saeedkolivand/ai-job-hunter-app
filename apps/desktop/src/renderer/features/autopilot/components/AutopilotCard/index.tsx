import {
  Briefcase,
  Check,
  ChevronUp,
  ExternalLink,
  Eye,
  Pause,
  Pencil,
  Play,
  RotateCcw,
  Sparkles,
  Trash2,
  Wand2,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import type { Autopilot, AutopilotFoundJob } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import {
  ActionMenu,
  type ActionMenuItem,
  Button,
  cn,
  ConfirmModal,
  GlassCard,
  Tag,
  transition,
} from '@ajh/ui';

import { type AutopilotRunState, RUN_STATE_LABEL } from '@/lib/machines/autopilot-run.machine';
import { MatchBand } from '@/lib/match-band';
import { timeAgo } from '@/lib/time';
import { TrustBadge } from '@/lib/trust-badge';
import { useInteractions, useOpenExternal, usePersistJob } from '@/services';

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
  rank_done: '★',
  cancelled: '⊘',
  complete: '✓',
};

const STATUS_TAG = 'rounded-full px-1.5 py-0.5 text-[8px] uppercase tracking-wider';

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
  const { t, i18n } = useTranslation();
  const openExternal = useOpenExternal();
  const persistJob = usePersistJob();
  const [showFound, setShowFound] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const headerRef = useRef<HTMLDivElement>(null);
  const listContainerRef = useRef<HTMLDivElement>(null);
  const foundJobs = ap.foundJobs ?? [];

  // Scroll-to-row + transient highlight target for `focusedJobUrl` (returning
  // from an Apply via Back). Kept in a ref (not state) since it isn't rendered;
  // `resolvePendingScroll` below clears it first so it can never double-fire
  // between the enter-animation and already-expanded-rAF paths.
  const pendingScrollUrlRef = useRef<string | null>(null);
  const pendingScrollRafRef = useRef<number | null>(null);
  const [highlightedUrl, setHighlightedUrl] = useState<string | null>(null);
  const highlightTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    return () => {
      if (highlightTimeoutRef.current) clearTimeout(highlightTimeoutRef.current);
      if (pendingScrollRafRef.current !== null) cancelAnimationFrame(pendingScrollRafRef.current);
    };
  }, []);

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

  // Idempotent: reads + clears `pendingScrollUrlRef` FIRST, so it's safe to
  // call from both the enter-animation completion and the already-expanded
  // rAF fallback below without double-scrolling or double-firing onFocusHandled.
  const resolvePendingScroll = useCallback(() => {
    const url = pendingScrollUrlRef.current;
    if (!url) return;
    pendingScrollUrlRef.current = null;
    const el = listContainerRef.current?.querySelector(`[data-job-url="${CSS.escape(url)}"]`);
    el?.scrollIntoView({ behavior: 'smooth', block: 'center' });
    setHighlightedUrl(url);
    if (highlightTimeoutRef.current) clearTimeout(highlightTimeoutRef.current);
    highlightTimeoutRef.current = setTimeout(() => setHighlightedUrl(null), 1500);
    onFocusHandled?.();
  }, [onFocusHandled]);

  // Tray "New jobs" / deep-link focus: open this card's found-jobs and scroll to
  // it, then tell the page to clear the focus so a later click re-triggers.
  // When `focusedJobUrl` is set (returning from an Apply via Back), defer the
  // scroll+highlight to that specific row: normally via the found-jobs panel's
  // `onAnimationComplete` (below) once its expand animation finishes, or — if
  // the panel was ALREADY expanded, so no enter animation fires — via a rAF
  // fallback here so the focus can never wedge.
  useEffect(() => {
    if (!focused) return;
    if (focusedJobUrl) {
      pendingScrollUrlRef.current = focusedJobUrl;
      // Functional update: reads the PRE-focus `showFound` without adding it as
      // a dependency (adding it would re-run this effect — and re-force the
      // panel open — on every manual toggle while still focused).
      setShowFound((wasExpanded) => {
        if (wasExpanded) pendingScrollRafRef.current = requestAnimationFrame(resolvePendingScroll);
        return true;
      });
    } else {
      setShowFound(true);
      headerRef.current?.scrollIntoView({ behavior: 'smooth', block: 'center' });
      onFocusHandled?.();
    }
  }, [focused, focusedJobUrl, onFocusHandled, resolvePendingScroll]);

  // #45 — relative last-run ("3 min ago") instead of an absolute timestamp.
  const lastRun = ap.lastRunAt
    ? timeAgo(ap.lastRunAt, Date.now(), i18n.language)
    : t('autopilot.wizard.never');

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

  // Toggle expand/collapse when clicking anywhere on the header row (if there
  // are found jobs). The actions cluster gets stopPropagation so its buttons
  // don't double-fire the toggle.
  const handleHeaderToggle = () => {
    if (foundJobs.length > 0) setShowFound((v) => !v);
  };
  const handleHeaderKeyDown = (e: React.KeyboardEvent) => {
    if ((e.key === 'Enter' || e.key === ' ') && foundJobs.length > 0) {
      e.preventDefault();
      setShowFound((v) => !v);
    }
  };
  const stopProp = (e: React.MouseEvent | React.KeyboardEvent) => e.stopPropagation();

  const handleJobClick = async (job: AutopilotFoundJob) => {
    void openExternal.mutate(job.url);
    // Also persist 'viewed' so the badge appears immediately and survives reload.
    try {
      await persistJob.mutateAsync({
        job: {
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

  return (
    <GlassCard className="flex flex-col gap-3">
      {/* Header row — click-to-expand when foundJobs exist */}
      <div
        ref={headerRef}
        className={cn(
          'flex items-center gap-4',
          foundJobs.length > 0 && 'cursor-pointer select-none rounded-lg'
        )}
        role={foundJobs.length > 0 ? 'button' : undefined}
        tabIndex={foundJobs.length > 0 ? 0 : undefined}
        aria-expanded={foundJobs.length > 0 ? showFound : undefined}
        aria-label={
          foundJobs.length > 0
            ? `${showFound ? t('autopilot.collapse') : t('autopilot.foundJobs')}: ${ap.name}`
            : undefined
        }
        onClick={handleHeaderToggle}
        onKeyDown={handleHeaderKeyDown}
      >
        {/* Status dot */}
        <div
          className={cn(
            'h-2 w-2 rounded-full shrink-0',
            paused
              ? 'bg-foreground/20'
              : running
                ? 'bg-amber-400 animate-pulse'
                : runState === 'error'
                  ? 'bg-red-400'
                  : 'bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.5)]'
          )}
        />

        {/* Info */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-0.5">
            <span className="text-sm font-semibold text-foreground/85 truncate">{ap.name}</span>
            <span className="text-[10px] text-foreground/30 font-mono bg-muted px-1.5 py-0.5 rounded">
              {(() => {
                const [firstBoard] = ap.target.boards;
                return ap.target.boards.length === 1
                  ? t(`jobs.boards.${firstBoard}`, { defaultValue: firstBoard ?? '' })
                  : t('autopilot.card.boardsCount', { count: ap.target.boards.length });
              })()}
            </span>
            <span className="text-[10px] text-foreground/30 bg-muted px-1.5 py-0.5 rounded capitalize">
              {ap.schedule.replace('_', ' ')}
            </span>
            {!running && (ap.runStatus === 'failed' || ap.runStatus === 'interrupted') && (
              <span
                className={cn(
                  'shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium',
                  ap.runStatus === 'failed'
                    ? 'bg-red-400/15 text-red-300'
                    : 'bg-amber-400/15 text-amber-300'
                )}
              >
                {t(
                  ap.runStatus === 'failed'
                    ? 'autopilot.badge.failed'
                    : 'autopilot.badge.interrupted'
                )}
              </span>
            )}
          </div>
          <div className="flex items-center gap-4 text-[10px] text-foreground/35">
            <span>"{ap.target.query}"</span>
            {ap.target.location && <span>· {ap.target.location}</span>}
            <span>
              · {t('autopilot.wizard.lastRun')} {lastRun}
            </span>
            <span>
              · {t('autopilot.wizard.found')} {foundJobs.length}
            </span>
          </div>
        </div>

        {/* Actions — stopPropagation so these don't toggle expand */}
        <div className="flex items-center gap-1.5 shrink-0" onClick={stopProp} onKeyDown={stopProp}>
          <Button
            onClick={onRun}
            disabled={running}
            className="flex items-center gap-1.5 rounded-lg bg-brand/10 px-2.5 py-1.5 text-[11px] font-medium text-brand-soft hover:bg-brand/20 transition-colors disabled:opacity-40 h-auto border-transparent"
          >
            {running ? <RotateCcw size={11} className="animate-spin" /> : <Play size={11} />}
            {running ? RUN_STATE_LABEL[runState] : t('autopilot.wizard.run')}
          </Button>
          {foundJobs.length > 0 && (
            <Button
              onClick={() => setShowFound((v) => !v)}
              aria-label={t('autopilot.foundJobs')}
              title={t('autopilot.foundJobs')}
              className={cn(
                'flex items-center gap-1 rounded-lg px-2 py-1.5 text-[11px] font-medium transition-colors h-auto border-transparent',
                showFound
                  ? 'bg-brand/15 text-brand-soft'
                  : 'bg-muted text-foreground/50 hover:text-foreground/80'
              )}
            >
              <Briefcase size={11} />
              {foundJobs.length}
            </Button>
          )}
          <ActionMenu label={t('autopilot.actions')} items={actionItems} />
        </div>
      </div>

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
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: 'auto' }}
            exit={{ opacity: 0, height: 0 }}
            transition={transition.fast}
            className="overflow-hidden"
            onAnimationComplete={resolvePendingScroll}
          >
            <div className="overflow-hidden rounded-lg border border-[var(--border-clear)] bg-card">
              <div className="flex items-center justify-between border-b border-[var(--border-clear)] px-3 py-2">
                <span className="text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/55">
                  {t('autopilot.foundJobs')} · {foundJobs.length}
                </span>
                <Button
                  variant="unstyled"
                  type="button"
                  onClick={() => setShowFound(false)}
                  aria-label={t('autopilot.collapse')}
                  title={t('autopilot.collapse')}
                  className="rounded p-0.5 text-foreground/30 transition-colors hover:text-foreground/70"
                >
                  <ChevronUp size={12} />
                </Button>
              </div>
              <div
                ref={listContainerRef}
                className="max-h-64 divide-y divide-[var(--border-clear)] overflow-y-auto"
              >
                {foundJobs.map((job, i) => (
                  <div
                    key={`${job.url}-${i}`}
                    data-job-url={job.url}
                    className={cn(
                      'flex flex-col gap-1 px-3 py-2 transition-colors hover:bg-muted',
                      highlightedUrl === job.url && 'ring-2 ring-inset ring-brand/60'
                    )}
                  >
                    <div className="flex items-center gap-2">
                      <Button
                        variant="unstyled"
                        type="button"
                        onClick={() => void handleJobClick(job)}
                        title={t('autopilot.viewJob')}
                        className="flex min-w-0 flex-1 items-center gap-2 text-left"
                      >
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-1.5">
                            <span className="truncate text-[11px] text-foreground/80">
                              {job.title}
                            </span>
                            {job.isNew && (
                              <span className="shrink-0 rounded-full bg-brand/15 px-1.5 py-0.5 text-[8px] font-semibold uppercase tracking-wider text-brand-soft">
                                {t('autopilot.badge.new')}
                              </span>
                            )}
                            {job.applied && (
                              <span className="flex shrink-0 items-center gap-0.5 rounded-full bg-emerald-400/15 px-1.5 py-0.5 text-[8px] font-semibold uppercase tracking-wider text-emerald-300">
                                <Check size={8} /> {t('autopilot.badge.applied')}
                              </span>
                            )}
                            {viewedUrls.has(job.url) && (
                              <Tag color="blue" icon={<Eye size={7} />} className={STATUS_TAG}>
                                {t('jobs.viewed')}
                              </Tag>
                            )}
                            {/* interactive=false: this whole row is already a <Button> (handleJobClick) —
                                a nested focusable popover trigger would be invalid HTML (button-in-button). */}
                            <TrustBadge
                              trust={job.trust}
                              className={STATUS_TAG}
                              interactive={false}
                            />
                          </div>
                          <div className="flex items-center gap-1.5 text-[10px] text-foreground/40">
                            <span className="truncate">{job.company}</span>
                            {job.location && <span className="truncate">· {job.location}</span>}
                          </div>
                        </div>
                        {typeof job.score === 'number' && (
                          <MatchBand value={job.score} variant="coverage" />
                        )}
                        <ExternalLink size={11} className="shrink-0 text-foreground/25" />
                      </Button>
                      <Button
                        onClick={() => onApply(job)}
                        title={t('autopilot.applyJob')}
                        className="flex shrink-0 items-center gap-1 rounded-lg border-transparent bg-brand/10 px-2 py-1 text-[10px] font-medium text-brand-soft transition-colors hover:bg-brand/20 h-auto"
                      >
                        <Wand2 size={10} /> {t('autopilot.applyJob')}
                      </Button>
                    </div>
                    {/* LLM-generated — always rendered as plain text, never markdown/HTML.
                        Visible "AI note" label (not just the aria-label) so sighted users get
                        the same "AI-generated, not fact" cue as the icon-only Sparkles gives
                        screen readers. Clamped to 2 lines — a verbose note gets a `title`
                        tooltip for the full text instead of dominating the compact row. */}
                    {job.assistantNotes && (
                      <div
                        role="note"
                        aria-label={t('autopilot.aiNote')}
                        className="ml-0.5 flex items-start gap-1.5 rounded-lg border border-brand/15 bg-brand/5 px-2.5 py-1.5"
                      >
                        <Sparkles size={10} className="mt-0.5 shrink-0 text-brand-soft" />
                        <div className="min-w-0 flex-1">
                          <span className="block text-fine-print font-semibold uppercase tracking-wide text-brand-soft">
                            {t('autopilot.aiNote')}
                          </span>
                          <p
                            title={job.assistantNotes}
                            className="line-clamp-2 text-[10px] leading-relaxed text-foreground/70"
                          >
                            {job.assistantNotes}
                          </p>
                        </div>
                      </div>
                    )}
                  </div>
                ))}
              </div>
            </div>
          </motion.div>
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
