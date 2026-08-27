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
import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import {
  ActionMenu,
  type ActionMenuItem,
  Button,
  cn,
  ConfirmModal,
  Dropdown,
  GlassCard,
  HoverPopover,
  Tag,
  transition,
  useNotification,
} from '@ajh/ui';

import { AgencyChip } from '@/components/job/AgencyChip';
import { ClusterSourceChips } from '@/components/job/ClusterSourceChips';
import { BoardSummaryChips } from '@/components/scrape/BoardSummaryChips';
import { useFormatRelativeTime } from '@/hooks/use-format-relative-time';
import { type AutopilotRunState, RUN_STATE_LABEL } from '@/lib/machines/autopilot-run.machine';
import { MatchBand, matchBandDescriptionKey, scoreTier } from '@/lib/match-band';
import { timeAgo } from '@/lib/time';
import { TrustBadge } from '@/lib/trust-badge';
import {
  useBoardsHealth,
  useInteractions,
  useMarkNotDuplicate,
  useOpenExternal,
  usePersistJob,
} from '@/services';

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

/**
 * Every metric a found-job score can be rendered as (ADR-020 addendum).
 *
 * A runtime tuple, not just a type: each variant owns an
 * `autopilot.scoreLabel.*` / `autopilot.scoreAbbr.*` key built by template
 * string, which TypeScript cannot check. Exported so the i18n test enumerates
 * the REAL set — restating the two strings there is how a third variant would
 * ship with no localized label.
 */
export const SCORE_VARIANTS = ['coverage', 'combined'] as const;
export type ScoreVariant = (typeof SCORE_VARIANTS)[number];

/**
 * The band variant a found job's score should render as. `'combined'` ONLY when
 * the backend says that job's score came from the semantic+ATS kernel — the
 * two metrics have different meanings AND different tier cut points, so showing
 * a keyword number on the combined scale (or vice versa) mislabels it. A job
 * that degraded back to keyword-only mid-run reports `'keyword'` and is
 * rendered as such (ADR-020 addendum).
 */
function scoreVariant(job: AutopilotFoundJob): ScoreVariant {
  return job.scoreSource === 'combined' ? 'combined' : 'coverage';
}

/**
 * Hover/screen-reader copy for a found job's score: WHICH metric it is
 * ("Keyword Coverage %" vs "Match %" — ADR-020 asked for this distinction and
 * it had never been surfaced), what the tier means, and — when provisional —
 * why the number is only an estimate.
 *
 * All three, not one of them. They answer different questions: the metric name
 * says what is being measured, the tier description says what "High" is
 * claiming, and `provisionalScoreHint` says how much to trust the number behind
 * it — so dropping any leaves a real gap. Composed here rather than inside
 * `MatchBand` because this wrapper owns the `title` and the sr-only span;
 * letting the band render its own would put a second `title` inside this one
 * (the inner wins on hover over the badge, hiding the rest) and announce twice.
 */
function scoreDetail(t: (key: string) => string, job: AutopilotFoundJob): string {
  const variant = scoreVariant(job);
  const label = t(`autopilot.scoreLabel.${variant}`);
  const tier = t(matchBandDescriptionKey(scoreTier(job.score ?? 0, variant).key, variant));
  const provisional = job.scoreProvisional ? ` ${t('autopilot.provisionalScoreHint')}` : '';
  return `${label}: ${tier}${provisional}`;
}

/** Badges that carry a hover/focus explainer now that the chip strip exists. */
const BADGE_HINT_KEY = {
  completedWithErrors: 'autopilot.badge.completedWithErrorsHint',
  needsConfig: 'autopilot.badge.needsConfigHint',
} as const;

/** A card's found-jobs sort choice — `'relevance'` is the stored rank order. */
export type FoundJobsSortBy = 'relevance' | 'newest' | 'oldest';

/**
 * View-side date sort for a card's found jobs. NEVER mutates `jobs` — the
 * persisted `Autopilot.foundJobs` order feeds AI-note recipient selection on
 * the backend (ADR-020), so this always sorts a fresh copy and returns it.
 * Exported so the mutation invariant + banding are directly unit-testable
 * without going through the component memo.
 *
 * Mirrors JobsPage's postedAt sort (JobsPage/index.tsx:288-300): a dated band
 * (sorted by `postedAt`) leads, an undated band trails instead of
 * interleaving, and a `url` tiebreak keeps equal-timestamp (or all-undated)
 * rows in a stable order across renders. Found jobs carry no `id` — `url` is
 * already the row's own render key (line ~630) and is unique per posting.
 * Deliberately NO `foundAt` (capture-time) fallback for undated rows — see
 * JobsPage:278-284: a just-scraped stale posting would otherwise jump above a
 * genuinely-recent one.
 */
export function sortFoundJobsByDate(
  jobs: AutopilotFoundJob[],
  sortBy: Exclude<FoundJobsSortBy, 'relevance'>
): AutopilotFoundJob[] {
  const byUrl = (x: AutopilotFoundJob, y: AutopilotFoundJob) =>
    x.url < y.url ? -1 : x.url > y.url ? 1 : 0;
  return [...jobs].sort((a, b) => {
    if (typeof a.postedAt !== 'number' || typeof b.postedAt !== 'number') {
      if (typeof a.postedAt === 'number') return -1; // a dated, b undated
      if (typeof b.postedAt === 'number') return 1; // b dated, a undated
      return byUrl(a, b); // both undated
    }
    const cmp = sortBy === 'oldest' ? a.postedAt - b.postedAt : b.postedAt - a.postedAt;
    return cmp || byUrl(a, b);
  });
}

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
  const formatRelativeTime = useFormatRelativeTime(t);
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
  // Persisted per-board outcome of the most recent run (PR B). Unlike the live
  // step log (below), this survives the run ending, so a zero/partial/failed
  // result stays explainable. Empty for the happy path + pre-summaries records.
  // Memoized because the `?? []` default is a fresh array each render, which
  // would re-run the health merge below on every render.
  const lastRunSummaries = useMemo(() => ap.lastRunSummaries ?? [], [ap.lastRunSummaries]);
  // Track B1 — the cross-run reliability verdict is read LIVE, never taken off
  // the stored record. `lastRunSummaries` is an immutable snapshot of one run;
  // health is standing state, so a persisted copy would keep asserting a streak
  // the store has since cleared (autopilot paused after a bad run, then a manual
  // scrape succeeds). Merged in here so the chips component stays presentational.
  const { data: boardHealth } = useBoardsHealth();
  const summariesWithHealth = useMemo(
    () =>
      boardHealth
        ? lastRunSummaries.map((s) => {
            const health = boardHealth.get(s.board);
            return health ? { ...s, health } : s;
          })
        : lastRunSummaries,
    [lastRunSummaries, boardHealth]
  );
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
                    <BoardSummaryChips summaries={summariesWithHealth} />
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
                {/* Per-card view-side sort (local state — see `sortBy` above):
                    each card sorts its own found-jobs list independently. */}
                <div className="flex items-center gap-1">
                  <Dropdown
                    options={[
                      { value: 'relevance', label: t('autopilot.sortRelevance') },
                      { value: 'newest', label: t('jobs.sortNewest') },
                      { value: 'oldest', label: t('jobs.sortOldest') },
                    ]}
                    value={sortBy}
                    onChange={(value) => setSortBy(value as FoundJobsSortBy)}
                    size="sm"
                    placeholder={t('jobs.sort')}
                    aria-label={t('jobs.sort')}
                  />
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
                            {/* Board coverage is uneven — several boards ship no publish
                                date — so absence renders nothing rather than "NaN ago".
                                Same helper + namespace the Jobs page uses for postedAt
                                (PostingListItem/index.tsx:121-122); the title carries the
                                absolute timestamp, mirroring ApplicationRow:231.
                                `typeof === 'number'`, not `job.postedAt &&` — the classic
                                0-&&-JSX footgun (a stray "0" text node) AND the one presence
                                contract shared with `sortFoundJobsByDate`'s dated/undated
                                banding below (which already treats 0 as dated). */}
                            {typeof job.postedAt === 'number' && (
                              <span
                                className="shrink-0 text-[10px] text-foreground/40"
                                title={new Date(job.postedAt).toLocaleString()}
                              >
                                · {formatRelativeTime(job.postedAt)}
                              </span>
                            )}
                          </div>
                          <div className="flex items-center gap-1.5 text-[10px] text-foreground/40">
                            <span className="truncate">{job.company}</span>
                            {job.location && <span className="truncate">· {job.location}</span>}
                          </div>
                        </div>
                        {typeof job.score === 'number' && (
                          // One wrapper for both cases so the metric label
                          // ("Keyword Coverage %" / "Match %") is always
                          // announced. A provisional score (audit root cause 6)
                          // is computed over a truncated aggregator snippet, so
                          // the detail pane's full-text re-score may differ —
                          // it additionally gets a muted band (ALL tiers,
                          // `muted`, not `subtle` — a provisional HIGH must read
                          // muted too, unlike `subtle`'s High-stays-bright
                          // contract), a "~" prefix and the caveat in the copy.
                          // The hover `title` plus an always-present sr-only
                          // span follows the TrustBadge non-interactive
                          // precedent (a `title` alone isn't reliably
                          // announced). No focusable HoverPopover — this whole
                          // row is already a <Button>; a focusable popover
                          // trigger nested in it would be invalid
                          // button-in-button HTML.
                          <span
                            className="inline-flex shrink-0 items-center gap-0.5"
                            title={scoreDetail(t, job)}
                          >
                            {mixedScoreSources && (
                              // The scale this number is on, shown only while
                              // the list actually mixes the two (see
                              // `mixedScoreSources`). aria-hidden because the
                              // sr-only span below already announces the full
                              // metric name — this is the sighted-user half of
                              // the same fact, not a second announcement.
                              <span
                                aria-hidden="true"
                                className="shrink-0 text-[8px] font-semibold uppercase tracking-wider text-foreground/40"
                              >
                                {t(`autopilot.scoreAbbr.${scoreVariant(job)}`)}
                              </span>
                            )}
                            {job.scoreProvisional && (
                              <span
                                aria-hidden="true"
                                className="text-[11px] leading-none text-foreground/35"
                              >
                                ~
                              </span>
                            )}
                            {/* describe={false}: this wrapper owns the copy —
                                the band's own `title` would otherwise win on
                                hover over the badge itself and hide the metric
                                label (and the provisional caveat), and its
                                sr-only suffix would double up with the one
                                below. Same caller-owns-richer-copy split as
                                RowMatchScore. */}
                            <MatchBand
                              value={job.score}
                              variant={scoreVariant(job)}
                              muted={job.scoreProvisional}
                              describe={false}
                            />
                            <span className="sr-only">: {scoreDetail(t, job)}</span>
                          </span>
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

                    {/* Cross-board cluster row (ADR-029) — agency marker, source
                        chips for other boards, and a split action. Kept OUTSIDE
                        the row's main <Button> above (interactive chips + split
                        would be invalid button-in-button HTML otherwise). */}
                    {(job.isAgency || (job.clusterMembers?.length ?? 0) > 1) && (
                      <div className="flex flex-wrap items-center gap-1.5 pl-0.5">
                        {job.isAgency && <AgencyChip className="px-1 py-0 text-[9px]" />}
                        <ClusterSourceChips
                          members={job.clusterMembers}
                          selfKey={job.clusterId}
                          selfUrl={job.url}
                        />
                        {(job.clusterMembers?.length ?? 0) > 1 && (
                          <Button
                            variant="unstyled"
                            data-testid={TEST_IDS.jobs.clusterSplitButton}
                            onClick={() => handleSplitCluster(job)}
                            disabled={split.isPending}
                            className="rounded px-1.5 py-0.5 text-[10px] text-foreground/50 transition-colors hover:text-foreground/80 focus-visible:ring-offset-1"
                          >
                            {t('jobs.cluster.notDuplicate')}
                          </Button>
                        )}
                      </div>
                    )}

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
