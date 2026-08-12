import { ChevronDown, ChevronRight, FileText } from 'lucide-react';
import { useState } from 'react';

import type { PipelineRunEvent, PipelineRunStatus, PipelineRunSummary } from '@ajh/shared/ipc';
import { useTranslation } from '@ajh/translations';
import { Button, cn } from '@ajh/ui';

import { useFormatRelativeTime } from '@/hooks/use-format-relative-time';
import { stoppedSuffix, UNKNOWN_STOPPED_SUFFIX } from '@/lib/stopped-reason';

/**
 * Status → the chip's colour family. `needsReview` gets its own (amber): it is
 * neither a success nor a failure, and colouring it green is the misreport the
 * terminal review exists to prevent.
 *
 * The BARE `-400` step, never `-300` and never an opacity suffix: only the bare
 * `-400` classes carry a light-scheme remap (`utilities.css`), and these chips
 * are small semibold captions on a tinted fill — the exact case the remap was
 * measured for.
 */
const STATUS_TONE: Record<PipelineRunStatus, string> = {
  running: 'border-blue-400/25 bg-blue-400/10 text-blue-400',
  completed: 'border-emerald-400/25 bg-emerald-400/10 text-emerald-400',
  needsReview: 'border-amber-400/25 bg-amber-400/10 text-amber-400',
  failed: 'border-red-400/25 bg-red-400/10 text-red-400',
  cancelled: 'border-white/10 bg-white/[0.04] text-foreground/55',
};

export interface PipelineRunsListProps {
  runs: PipelineRunSummary[];
  /** Open one run's report. Omit to render the list read-only. */
  onOpenReport?: (runId: string) => void;
  /**
   * Persisted stage trail per run, when the caller has fetched the run's detail
   * (`get` carries `events`). A run with no entry here shows the "live progress
   * only" note instead of a fabricated timeline — the list endpoint returns
   * SUMMARIES, so a trail this component didn't receive genuinely isn't known.
   */
  eventsByRun?: Record<string, PipelineRunEvent[] | undefined>;
  /** Called when a row's timeline is expanded — lets the host fetch that run's
   *  detail lazily instead of loading three trails up front. */
  onExpand?: (runId: string) => void;
  className?: string;
}

/**
 * The per-posting run history — at most three, newest first, which is exactly
 * what the backend retains.
 *
 * Two things the contract makes the list responsible for saying out loud:
 * every run of a posting merges into ONE `ai_generations` row, so only the
 * newest run's document still exists; and `stoppedReason` is nullable on a
 * terminal run, so a missing one degrades to nothing at all rather than to
 * "Finished" — `status` is the authority.
 */
export function PipelineRunsList({
  runs,
  onOpenReport,
  eventsByRun,
  onExpand,
  className,
}: PipelineRunsListProps) {
  const { t } = useTranslation();
  const formatRelative = useFormatRelativeTime(t);
  const [expanded, setExpanded] = useState<string | null>(null);

  if (runs.length === 0) {
    return (
      <p className={cn('text-[11px] text-foreground/40', className)}>{t('pipeline.runs.empty')}</p>
    );
  }

  const toggle = (runId: string) => {
    const next = expanded === runId ? null : runId;
    setExpanded(next);
    if (next) onExpand?.(next);
  };

  return (
    <div className={className}>
      <h3 className="mb-2 text-[11px] font-semibold uppercase tracking-[0.14em] text-foreground/45">
        {t('pipeline.runs.title')}
      </h3>
      <ul className="space-y-1.5">
        {runs.map((run, index) => {
          const suffix = stoppedSuffix(run.stoppedReason);
          const open = expanded === run.runId;
          const events = eventsByRun?.[run.runId];
          // Only referenced while the region is actually rendered — an
          // `aria-controls` pointing at absent DOM is a broken reference.
          const timelineId = `pipeline-timeline-${run.runId}`;
          return (
            <li
              key={run.runId}
              className="rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2"
            >
              <div className="flex flex-wrap items-center gap-2">
                <span
                  className={cn(
                    'shrink-0 rounded-full border px-2 py-0.5 text-[10px] font-semibold',
                    STATUS_TONE[run.status]
                  )}
                >
                  {t(`pipeline.status.${run.status}`)}
                </span>
                <span className="text-[10px] uppercase tracking-[0.14em] text-foreground/45">
                  {t(`generationDepth.option.${run.depth}.label`)}
                </span>
                <span className="text-[10px] text-foreground/40">
                  {formatRelative(run.startedAt)}
                </span>
                <span className="flex-1" />
                {onOpenReport && (
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={() => onOpenReport(run.runId)}
                    className="shrink-0 text-[10px]"
                  >
                    <FileText size={10} />
                    {t('pipeline.runs.open')}
                  </Button>
                )}
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  aria-expanded={open}
                  aria-controls={open ? timelineId : undefined}
                  onClick={() => toggle(run.runId)}
                  className="shrink-0 text-[10px]"
                >
                  {open ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
                  {open ? t('pipeline.runs.closeTimeline') : t('pipeline.runs.openTimeline')}
                </Button>
              </div>

              <p className="mt-1 text-[10px] text-foreground/40">
                {/* An absent reason contributes NOTHING — never "Finished". */}
                {suffix && `${t(`pipeline.stopped.${suffix ?? UNKNOWN_STOPPED_SUFFIX}`)} · `}
                {t('pipeline.runs.metrics', {
                  calls: run.metrics.calls ?? 0,
                  repairs: run.metrics.repairRounds ?? 0,
                })}
                {run.metrics.reverted && ` · ${t('pipeline.runs.reverted')}`}
              </p>

              {open &&
                (events?.length ? (
                  <ol id={timelineId} className="mt-2 space-y-1 border-l border-white/10 pl-3">
                    {events.map((event) => (
                      <li key={event.seq} className="text-[10px] text-foreground/50">
                        <span className="text-foreground/70">
                          {t(`pipeline.stage.${event.stage}`, { defaultValue: event.stage })}
                        </span>{' '}
                        {/* `phase` is a Rust enum name (`start`/`finish`/`error`),
                            not copy — a future phase this build predates falls
                            back to the raw token rather than a missing-key. */}
                        · {t(`pipeline.phase.${event.phase}`, { defaultValue: event.phase })}
                      </li>
                    ))}
                  </ol>
                ) : (
                  <p id={timelineId} className="mt-2 text-[10px] text-foreground/35">
                    {events ? t('pipeline.timeline.empty') : t('pipeline.timeline.liveOnly')}
                  </p>
                ))}

              {index === 0 && runs.length > 1 && (
                <p className="mt-1.5 text-[10px] text-foreground/30">
                  {t('pipeline.runs.documentNote')}
                </p>
              )}
            </li>
          );
        })}
      </ul>
      <p className="mt-1.5 text-[10px] text-foreground/30">{t('pipeline.runs.retentionNote')}</p>
    </div>
  );
}
