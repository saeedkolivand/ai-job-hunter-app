import {
  Briefcase,
  Check,
  ChevronUp,
  ExternalLink,
  Eye,
  Info,
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

import type { Autopilot, AutopilotFoundJob, AutopilotRunStatus } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import {
  ActionMenu,
  type ActionMenuItem,
  Button,
  cn,
  ConfirmModal,
  GlassCard,
  HoverPopover,
  Tag,
  transition,
} from '@ajh/ui';

import { BoardSummaryChips } from '@/components/scrape/BoardSummaryChips';
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

/**
 * Persisted run-outcome → badge label key + color. A `Partial` map IS the
 * graceful fallback: a happy `completed`/`inProgress` — or any unknown/future
 * `runStatus` — is simply absent from the map, so no badge renders and nothing
 * ever prints a raw enum string. `failed` reads as an error (red);
 * `completedWithErrors` (some boards failed/truncated) and `interrupted` read as
 * warnings (amber).
 */
const RUN_STATUS_BADGE: Partial<
  Record<AutopilotRunStatus, { labelKey: string; className: string }>
> = {
  failed: { labelKey: 'autopilot.badge.failed', className: 'bg-red-400/15 text-red-300' },
  completedWithErrors: {
    labelKey: 'autopilot.badge.completedWithErrors',
    className: 'bg-amber-400/15 text-amber-300',
  },
  interrupted: {
    labelKey: 'autopilot.badge.interrupted',
    className: 'bg-amber-400/15 text-amber-300',
  },
};

/**
 * Cry-wolf guard (PR B carry-over 2): a `failed` run whose boards were ALL merely
 * skipped (needs-login / needs-keys / needs-company) — none actually errored —
 * isn't a failure, it's an unconfigured run. Present it neutrally + actionably
 * ("needs configuration") instead of a red "Failed", with the per-board chip
 * strip below spelling out exactly what to configure.
 */
const NEEDS_CONFIG_BADGE = {
  labelKey: 'autopilot.badge.needsConfig',
  className: 'bg-foreground/[0.06] text-foreground/70',
};

/** Badges that carry a hover/focus explainer now that the chip strip exists. */
const BADGE_HINT_KEY = {
  completedWithErrors: 'autopilot.badge.completedWithErrorsHint',
  needsConfig: 'autopilot.badge.needsConfigHint',
} as const;

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
  // Persisted per-board outcome of the most recent run (PR B). Unlike the live
  // step log (below), this survives the run ending, so a zero/partial/failed
  // result stays explainable. Empty for the happy path + pre-summaries records.
  const lastRunSummaries = ap.lastRunSummaries ?? [];
  // Discoverability guard: `runStatus` doesn't escalate for a board that's
  // merely `skipped`/`truncated` beside an otherwise-succeeding board (e.g.
  // "Xing · needs login" next to a clean LinkedIn run reads as plain
  // `completed` — no colored badge at all), so the collapsed info trigger is
  // the ONLY surviving signal and must carry its own amber tone. An
  // informational `note` (e.g. a broadened-location hint) does NOT count —
  // it's benign, not a cry-wolf amber.
  const boardsDegraded = lastRunSummaries.some((s) => s.error || s.skipped || s.truncated);
  // Cry-wolf guard (PR B carry-over 2): a `failed` run whose boards were ALL
  // merely skipped (none errored) is an UNCONFIGURED run, not a failure.
  const needsConfig =
    ap.runStatus === 'failed' &&
    lastRunSummaries.length > 0 &&
    lastRunSummaries.every((s) => Boolean(s.skipped) && !s.error);
  // Persisted run-outcome badge (failed / completedWithErrors / interrupted).
  // `needsConfig` overrides the red `failed` badge with a neutral one; otherwise
  // undefined for the happy path and any unknown/future status — the explicit
  // graceful fallback (renders nothing rather than a raw enum).
  const runStatusBadge = needsConfig
    ? NEEDS_CONFIG_BADGE
    : ap.runStatus
      ? RUN_STATUS_BADGE[ap.runStatus]
      : undefined;
  // Optional hover/focus explainer for the neutral/amber badges — the
  // per-board detail itself lives behind the info icon next to "Found N".
  const badgeHintKey = needsConfig
    ? BADGE_HINT_KEY.needsConfig
    : ap.runStatus === 'completedWithErrors'
      ? BADGE_HINT_KEY.completedWithErrors
      : undefined;

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
            {!running &&
              runStatusBadge &&
              (badgeHintKey ? (
                // stopProp wrapper keeps a click/Enter on the badge from toggling
                // the card's found-jobs panel; Escape-to-close still reaches the
                // popover (its handler sits between the trigger and this wrapper).
                <span onClick={stopProp} onKeyDown={stopProp} className="inline-flex shrink-0">
                  <HoverPopover
                    placement="top"
                    ariaLabel={t(runStatusBadge.labelKey)}
                    contentClassName="max-w-[240px] rounded-lg border border-[var(--border-clear)] bg-card px-3 py-2 text-[11px] leading-relaxed text-foreground/70 shadow-lg"
                    trigger={
                      <span
                        tabIndex={0}
                        className={cn(
                          'inline-flex cursor-help rounded px-1.5 py-0.5 text-[10px] font-medium outline-none focus-visible:ring-2 focus-visible:ring-brand/50',
                          runStatusBadge.className
                        )}
                      >
                        {t(runStatusBadge.labelKey)}
                      </span>
                    }
                  >
                    {t(badgeHintKey)}
                  </HoverPopover>
                </span>
              ) : (
                <span
                  className={cn(
                    'shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium',
                    runStatusBadge.className
                  )}
                >
                  {t(runStatusBadge.labelKey)}
                </span>
              ))}
          </div>
          <div className="flex items-center gap-4 text-[10px] text-foreground/35">
            <span>"{ap.target.query}"</span>
            {ap.target.location && <span>· {ap.target.location}</span>}
            <span>
              · {t('autopilot.wizard.lastRun')} {lastRun}
            </span>
            {/* gap-1 sub-group: the info trigger reads as an annotation ON the
                found-count, not a stray icon floating at the row's gap-4. */}
            <span className="inline-flex items-center gap-1">
              <span>
                · {t('autopilot.wizard.found')} {foundJobs.length}
              </span>
              {!running && lastRunSummaries.length > 0 && (
                // stopProp wrapper: same reason as the badge popover above —
                // keeps this from also toggling the card's found-jobs panel.
                // The trigger is a real <Button> (native focus, no tabIndex
                // needed), so the HoverPopover's focus-opens-it mechanic is
                // keyboard-reachable by default (Tab to it, Esc to close)
                // without extra wiring.
                <span onClick={stopProp} onKeyDown={stopProp} className="inline-flex shrink-0">
                  <HoverPopover
                    placement="top"
                    ariaLabel={t('autopilot.boardResults.infoLabel')}
                    contentClassName="max-w-[280px] rounded-lg border border-[var(--border-clear)] bg-card px-3 py-2 shadow-lg"
                    trigger={
                      <Button
                        variant="unstyled"
                        type="button"
                        aria-label={t('autopilot.boardResults.infoLabel')}
                        title={t('autopilot.boardResults.infoLabel')}
                        data-degraded={boardsDegraded}
                        className={cn(
                          // ≥20px hit target (14px icon + p-1). Discoverability:
                          // a degraded board (error/skipped/truncated) escalates
                          // to the same amber the warning badges use, at
                          // near-full opacity — it's the ONLY surviving signal
                          // once runStatus itself doesn't escalate (e.g. one
                          // skipped board beside an otherwise-clean run). Clean
                          // runs rest at the documented /70 floor, never lower.
                          'inline-flex items-center justify-center rounded p-1 transition-colors',
                          // No hover shade on the degraded state: amber-200
                          // isn't in tokens.css's light-scheme remap (only
                          // 300/400/500 are), so it'd render raw pale amber on
                          // light (~1.2:1). Already near-full opacity; the
                          // popover itself is the real hover feedback.
                          boardsDegraded
                            ? 'text-amber-300'
                            : 'text-foreground/70 hover:text-foreground'
                        )}
                      >
                        <Info size={14} />
                      </Button>
                    }
                  >
                    <BoardSummaryChips summaries={lastRunSummaries} />
                  </HoverPopover>
                </span>
              )}
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
                  className="rounded p-1 text-foreground/30 transition-colors hover:text-foreground/70"
                >
                  <ChevronUp size={14} />
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
                        {typeof job.score === 'number' &&
                          (job.scoreProvisional ? (
                            // Provisional score (audit root cause 6): computed
                            // over a truncated aggregator snippet, so the detail
                            // pane's full-text re-score may differ. Mark it
                            // honestly — a muted band (ALL tiers, `muted`, not
                            // `subtle` — a provisional HIGH must read muted too,
                            // unlike `subtle`'s High-stays-bright contract) + "~"
                            // prefix + a hover `title` AND an always-present
                            // sr-only span (the TrustBadge non-interactive
                            // precedent: a `title` alone isn't reliably
                            // announced). No focusable HoverPopover — this whole
                            // row is already a <Button>; a focusable popover
                            // trigger nested in it would be invalid
                            // button-in-button HTML (same reason TrustBadge
                            // above renders interactive=false).
                            <span
                              className="inline-flex shrink-0 items-center gap-0.5"
                              title={t('autopilot.provisionalScoreHint')}
                            >
                              <span
                                aria-hidden="true"
                                className="text-[11px] leading-none text-foreground/35"
                              >
                                ~
                              </span>
                              <MatchBand value={job.score} variant="coverage" muted />
                              <span className="sr-only">
                                : {t('autopilot.provisionalScoreHint')}
                              </span>
                            </span>
                          ) : (
                            <MatchBand value={job.score} variant="coverage" />
                          ))}
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
