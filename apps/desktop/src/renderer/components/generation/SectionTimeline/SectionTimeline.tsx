import { AlertTriangle, Check, CheckCircle2, Circle, Loader2, Search } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { PIPELINE_SECTION_EXPERIENCE_PREFIX, type PipelineSectionKey } from '@ajh/shared';
import { type TFunction, useTranslation } from '@ajh/translations';
import { Tag, type TagProps } from '@ajh/ui';

import type {
  PipelineSectionState,
  PipelineSectionStates,
} from '@/lib/machines/resume-pipeline.machine';

export interface SectionTimelineProps {
  /** The live per-section map from the run session. Empty renders nothing. */
  states: PipelineSectionStates;
  className?: string;
}

/** Icon + tint per ladder state. Bare `-400` tints only: an opacity-suffixed
 *  colour utility escapes the light-theme remap (the Phase 3 contrast bug). */
const STATE_STYLE: Record<
  PipelineSectionState,
  { Icon: typeof Circle; tint: string; tag: TagProps['color']; spin?: true }
> = {
  queued: { Icon: Circle, tint: 'text-foreground/25', tag: 'default' },
  generating: { Icon: Loader2, tint: 'text-brand-soft', tag: 'processing', spin: true },
  done: { Icon: Check, tint: 'text-foreground/60', tag: 'default' },
  checking: { Icon: Search, tint: 'text-brand-soft', tag: 'processing' },
  needsChanges: { Icon: AlertTriangle, tint: 'text-amber-400', tag: 'warning' },
  repaired: { Icon: Check, tint: 'text-emerald-400', tag: 'success' },
  clean: { Icon: CheckCircle2, tint: 'text-emerald-400', tag: 'success' },
};

/**
 * States in which a section's text EXISTS — everything past `generating`.
 *
 * What the header counts, and deliberately not "finished": a checked or
 * repaired section is still written, and a section the run later flags has not
 * un-written itself.
 */
const WRITTEN_STATES: ReadonlySet<PipelineSectionState> = new Set<PipelineSectionState>([
  'done',
  'checking',
  'needsChanges',
  'repaired',
  'clean',
]);

/**
 * A section key rendered for a human.
 *
 * `experience:<i>` indexes the strategy's company roster in generation order,
 * and the renderer has no roster to name — the wire is content-free by design,
 * so the company NAME is not on it. The entry is numbered from 1 rather than
 * labelled with a guess.
 *
 * The index is guarded rather than trusted: the runtime grammar
 * (`isPipelineSectionKey`) is enforced at the emitter, but this type is a
 * template literal — `experience:${number}` also accepts `NaN` and `Infinity`
 * from TypeScript's point of view, and "Experience NaN" is a worse answer than
 * an unnumbered entry.
 */
export function sectionLabel(key: PipelineSectionKey, t: TFunction): string {
  if (key.startsWith(PIPELINE_SECTION_EXPERIENCE_PREFIX)) {
    const index = Number(key.slice(PIPELINE_SECTION_EXPERIENCE_PREFIX.length));
    return t('pipeline.section.experienceEntry', {
      index: Number.isFinite(index) ? index + 1 : '?',
    });
  }
  return t(`pipeline.section.${key}`, { defaultValue: key });
}

/**
 * Live per-section checklist for a MAX-depth run.
 *
 * Renders exactly the sections the run has REPORTED, in arrival order — which
 * is generation order (summary → skills → each company → projects → education).
 * There are no placeholder rows for sections still to come: the roster depends
 * on the strategy's company plan and never reaches the renderer, so a
 * pre-drawn list would be an invention. See `foldSectionStates` for which
 * states the event stream can prove; a section holds its last proven state
 * rather than advancing on a guess.
 *
 * Empty at quality depth (no section events exist) and for a reconnected run
 * (the persisted trail carries no section key), so the host can render it
 * unconditionally.
 */
export function SectionTimeline({ states, className }: SectionTimelineProps) {
  const { t } = useTranslation();
  const rows = Object.entries(states) as [PipelineSectionKey, PipelineSectionState][];
  const written = rows.filter(([, state]) => WRITTEN_STATES.has(state)).length;

  /**
   * Milestone announcer, the `PrepApplicationPanel` pattern (its
   * `prevStatusRef` + sr-only `role="status"` span).
   *
   * The host's stage caption CANNOT do this job, which an earlier version of
   * this comment claimed it did: a section event carries the STAGE's
   * `index`/`total`, identical for every section, so the caption's text is
   * frozen for the whole `sections` stage — the run's longest phase — and an
   * unchanged live region announces nothing. Without this a screen-reader user
   * hears one utterance and then silence for up to two hours.
   *
   * Fires on a TRANSITION only (the ref guard), and announces the LAST changed
   * section when several move at once — `validate`'s start moves every written
   * section to `checking` in one go, and eight utterances for one stage
   * boundary is the noise this pattern exists to avoid.
   */
  const previous = useRef<PipelineSectionStates>({});
  const [announcement, setAnnouncement] = useState('');
  useEffect(() => {
    let latest: [PipelineSectionKey, PipelineSectionState] | null = null;
    for (const [key, state] of Object.entries(states) as [
      PipelineSectionKey,
      PipelineSectionState,
    ][]) {
      if (previous.current[key] === state) continue;
      latest = [key, state];
    }
    previous.current = states;
    if (!latest) return;
    setAnnouncement(
      t('pipeline.section.announce', {
        section: sectionLabel(latest[0], t),
        state: t(`pipeline.section.state.${latest[1]}`),
      })
    );
  }, [states, t]);

  if (rows.length === 0) return null;

  return (
    <div className={className}>
      <div className="mb-2 flex items-baseline justify-between gap-2">
        <h3 className="text-[11px] font-semibold uppercase tracking-[0.14em] text-foreground/45">
          {t('pipeline.section.title')}
        </h3>
        {/*
          The visible liveness cue. The stage counter above cannot move during
          the sections stage (it counts STAGES), so without a number that
          changes per section a max run reads as stalled for its longest phase.
          Counted against the sections SEEN, never against a total: the roster
          is not on the wire, and "3 of 9" would be a number this build made up.
        */}
        <span className="text-[10px] tabular-nums text-foreground/40">
          {t('pipeline.section.progress', { written, count: rows.length })}
        </span>
      </div>

      {/* One utterance per section transition; the rows themselves are not a
          live region, so nothing is re-read when a later section moves. */}
      <span role="status" aria-live="polite" className="sr-only">
        {announcement}
      </span>

      {/*
        A real list: AT announces "list, N items" and offers list navigation.
        (It was `role="group"` to avoid live-region noise — a rationale that
        role never controlled; the announcer above is what handles that.)
      */}
      <ul
        aria-label={t('pipeline.section.title')}
        className="space-y-1"
        data-testid="section-timeline"
      >
        {rows.map(([key, state]) => {
          const { Icon, tint, tag, spin } = STATE_STYLE[state];
          const label = sectionLabel(key, t);
          const stateLabel = t(`pipeline.section.state.${state}`);
          return (
            <li
              key={key}
              data-section={key}
              data-state={state}
              className="flex items-center gap-2 rounded-lg border border-[var(--border-clear)] bg-card px-3 py-1.5"
            >
              <Icon
                size={12}
                className={`shrink-0 ${tint}${spin ? ' animate-spin' : ''}`}
                aria-hidden="true"
              />
              <span className="min-w-0 flex-1 truncate text-[11px] font-medium text-foreground/80">
                {label}
              </span>
              {/* The state is TEXT, not colour alone — the tint repeats it. */}
              <Tag color={tag} className="shrink-0 text-[9px]">
                {stateLabel}
              </Tag>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
