import { AlertTriangle, Check, CheckCircle2, Circle, Loader2, Search } from 'lucide-react';

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
 * A section key rendered for a human.
 *
 * `experience:<i>` indexes the strategy's company roster in generation order,
 * and the renderer has no roster to name — the wire is content-free by design,
 * so the company NAME is not on it. The entry is numbered from 1 rather than
 * labelled with a guess.
 */
export function sectionLabel(key: PipelineSectionKey, t: TFunction): string {
  if (key.startsWith(PIPELINE_SECTION_EXPERIENCE_PREFIX)) {
    const index = Number(key.slice(PIPELINE_SECTION_EXPERIENCE_PREFIX.length));
    return t('pipeline.section.experienceEntry', { index: index + 1 });
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
  if (rows.length === 0) return null;

  return (
    <div className={className}>
      <h3 className="mb-2 text-[11px] font-semibold uppercase tracking-[0.14em] text-foreground/45">
        {t('pipeline.section.title')}
      </h3>
      {/*
        A `group`, not a live region: the list changes on every section event
        and an `aria-live` list would re-announce every row each time. The
        host's stage caption is the run's single announcer, exactly as the prep
        checklist does it.
      */}
      <ul
        role="group"
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
