import { AlarmClock, Check } from 'lucide-react';

import type { Application } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { Button, cn } from '@ajh/ui';

import {
  overdueCount,
  PIPELINE_GROUPS,
  pipelineCounts,
  type PipelineGroupId,
} from '@/features/applications/lib/pipeline';

interface PipelineStripProps {
  /** The full (unfiltered) list — counts always describe every application. */
  applications: readonly Application[];
  /** Currently selected group id, or `null` when no stage filter is active. */
  active: string | null;
  /** Fires with the next group id, or `null` when the active card is re-clicked. */
  onSelect: (groupId: PipelineGroupId | null) => void;
  /** Bring the overdue follow-ups to the top of the list (next-action sort). */
  onShowOverdue: () => void;
}

/**
 * The six-card pipeline summary above the Applications list. Each card is a
 * toggle filter for its stage group (click again to clear) and every card is
 * always rendered — a zero is information too ("nothing in Interviewing").
 *
 * Selection is expressed with a **ring** (an uncontested `box-shadow`) plus a
 * check glyph, NOT a background/border swap: `.surface-card`'s own background
 * and border are unlayered, so a layered `bg-*`/`border-*` utility loses to them
 * and the selected card painted pixel-identical to an unselected one. The check
 * is the non-colour cue, so the state survives colour-vision deficiency too.
 */
export function PipelineStrip({
  applications,
  active,
  onSelect,
  onShowOverdue,
}: PipelineStripProps) {
  const { t } = useTranslation();
  const counts = pipelineCounts(applications);
  const overdue = overdueCount(applications);
  const activeGroup = PIPELINE_GROUPS.find((g) => g.id === active);

  // `closed` is the only group that aggregates, so it is the only one needing a
  // label of its own; the rest reuse the canonical stage names.
  const groupLabel = (id: PipelineGroupId) =>
    id === 'closed' ? t('applications.pipeline.closed') : t(`applications.stages.${id}` as const);

  return (
    <div className="@container">
      <div
        role="group"
        aria-label={t('applications.pipeline.aria')}
        className="grid grid-cols-3 gap-2 @2xl:grid-cols-6"
      >
        {PIPELINE_GROUPS.map((group) => {
          const selected = active === group.id;
          return (
            <Button
              key={group.id}
              variant="unstyled"
              aria-pressed={selected}
              data-testid={TEST_IDS.applications.pipelineCard}
              data-group={group.id}
              onClick={() => onSelect(selected ? null : group.id)}
              className={cn(
                'surface-card flex min-h-11 flex-col items-start justify-center gap-1 px-3 py-2 text-left transition-shadow',
                selected
                  ? 'ring-2 ring-inset ring-brand/70'
                  : 'hover:ring-1 hover:ring-inset hover:ring-foreground/15'
              )}
            >
              <span className="flex w-full min-w-0 items-center gap-1">
                {/* Fixed-width slot: the glyph appears/disappears without shifting
                    the label, and carries the state independently of colour. */}
                <span className="flex w-3 shrink-0 justify-center">
                  {selected && <Check size={11} className="text-brand-soft" aria-hidden="true" />}
                </span>
                <span className="truncate text-xs font-semibold uppercase tracking-wider text-foreground/70">
                  {groupLabel(group.id)}
                </span>
              </span>
              <span
                className={cn(
                  'pl-4 text-lg font-semibold leading-none',
                  selected ? 'text-brand-soft' : 'text-foreground/85'
                )}
              >
                {counts[group.id]}
              </span>
            </Button>
          );
        })}
      </div>

      {/* Toggling a card silently re-filters the list below; announce the new
          scope so the change is not sighted-only. */}
      <span role="status" aria-live="polite" className="sr-only">
        {activeGroup
          ? t('applications.pipeline.filtered', { stage: groupLabel(activeGroup.id) })
          : t('applications.pipeline.filterCleared')}
      </span>

      {overdue > 0 && (
        <div className="mt-2 flex justify-end">
          {/* Actionable, not decorative: the whole point of surfacing a count is
              being able to reach those rows. Sorts by next action. */}
          <Button
            variant="unstyled"
            onClick={onShowOverdue}
            className={cn(
              'inline-flex items-center gap-1 rounded-full border border-red-400/40 px-2 py-0.5',
              'text-xs font-semibold text-red-400 transition-shadow',
              'hover:ring-1 hover:ring-inset hover:ring-red-400/40'
            )}
          >
            <AlarmClock size={11} aria-hidden="true" />
            {t('applications.pipeline.overdue', { n: overdue })}
          </Button>
        </div>
      )}
    </div>
  );
}
