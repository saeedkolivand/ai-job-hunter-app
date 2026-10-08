import { Briefcase, Info, Play, RotateCcw } from 'lucide-react';
import { useMemo } from 'react';

import type { Autopilot } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { ActionMenu, type ActionMenuItem, Button, cn, HoverPopover } from '@ajh/ui';

import { BoardSummaryChips } from '@/components/scrape/BoardSummaryChips';
import { type AutopilotRunState, RUN_STATE_LABEL } from '@/lib/machines/autopilot-run.machine';
import { timeAgo } from '@/lib/time';
import { useBoardsHealth } from '@/services';

import { describeRunOutcome } from './run-outcome';

interface Props {
  autopilot: Autopilot;
  runState: AutopilotRunState;
  running: boolean;
  foundCount: number;
  showFound: boolean;
  actionItems: ActionMenuItem[];
  headerRef: React.RefObject<HTMLDivElement | null>;
  onRun: () => void;
  onToggleFound: () => void;
}

/** Schedule id → its `autopilot.wizard.schedule.*` label key (never print the raw id). */
const SCHEDULE_LABEL_KEY: Record<Autopilot['schedule'], string> = {
  manual: 'autopilot.wizard.schedule.manual',
  hourly: 'autopilot.wizard.schedule.hourly',
  daily: 'autopilot.wizard.schedule.daily',
  twice_daily: 'autopilot.wizard.schedule.twiceDaily',
};

const stopProp = (e: React.MouseEvent | React.KeyboardEvent) => e.stopPropagation();

/** The card's header row — click-to-expand when found jobs exist. */
export function AutopilotCardHeader({
  autopilot: ap,
  runState,
  running,
  foundCount,
  showFound,
  actionItems,
  headerRef,
  onRun,
  onToggleFound,
}: Props) {
  const { t, i18n } = useTranslation();
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
  const { badge: runBadge, hintKey: badgeHintKey } = describeRunOutcome(
    ap.runStatus,
    lastRunSummaries
  );

  // #45 — relative last-run ("3 min ago") instead of an absolute timestamp.
  const lastRun = ap.lastRunAt
    ? timeAgo(ap.lastRunAt, Date.now(), i18n.language)
    : t('autopilot.wizard.never');
  const paused = ap.status === 'paused';
  const hasFound = foundCount > 0;

  // Toggle expand/collapse when clicking anywhere on the header row (if there
  // are found jobs). The actions cluster gets stopPropagation so its buttons
  // don't double-fire the toggle.
  const handleHeaderToggle = () => {
    if (hasFound) onToggleFound();
  };
  const handleHeaderKeyDown = (e: React.KeyboardEvent) => {
    if ((e.key === 'Enter' || e.key === ' ') && hasFound) {
      e.preventDefault();
      onToggleFound();
    }
  };

  return (
    <div
      ref={headerRef}
      className={cn('flex items-center gap-4', hasFound && 'cursor-pointer select-none rounded-lg')}
      role={hasFound ? 'button' : undefined}
      tabIndex={hasFound ? 0 : undefined}
      aria-expanded={hasFound ? showFound : undefined}
      aria-label={
        hasFound
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
          <span className="text-[10px] text-foreground/30 bg-muted px-1.5 py-0.5 rounded">
            {t(SCHEDULE_LABEL_KEY[ap.schedule])}
          </span>
          {!running &&
            runBadge &&
            (badgeHintKey ? (
              // stopProp wrapper keeps a click/Enter on the badge from toggling
              // the card's found-jobs panel; Escape-to-close still reaches the
              // popover (its handler sits between the trigger and this wrapper).
              <span onClick={stopProp} onKeyDown={stopProp} className="inline-flex shrink-0">
                <HoverPopover
                  placement="top"
                  ariaLabel={t(runBadge.labelKey)}
                  contentClassName="max-w-[240px] rounded-lg border border-[var(--border-clear)] bg-card px-3 py-2 text-[11px] leading-relaxed text-foreground/70 shadow-lg"
                  trigger={
                    <span
                      tabIndex={0}
                      className={cn(
                        'inline-flex cursor-help rounded px-1.5 py-0.5 text-[10px] font-medium outline-none focus-visible:ring-2 focus-visible:ring-brand/50',
                        runBadge.className
                      )}
                    >
                      {t(runBadge.labelKey)}
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
                  runBadge.className
                )}
              >
                {t(runBadge.labelKey)}
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
              · {t('autopilot.wizard.found')} {foundCount}
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
        {hasFound && (
          <Button
            onClick={onToggleFound}
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
            {foundCount}
          </Button>
        )}
        <ActionMenu label={t('autopilot.actions')} items={actionItems} />
      </div>
    </div>
  );
}
