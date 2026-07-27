import { AlarmClock } from 'lucide-react';

import type { Application } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, cn, Tag } from '@ajh/ui';

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
}

/**
 * The six-card pipeline summary above the Applications list. Each card is a
 * toggle filter for its stage group (click again to clear) and every card is
 * always rendered — a zero is information too ("nothing in Interviewing").
 *
 * Restraint rules: flat `.surface-card` (the GlassCard `surface` tone), no new
 * shadow, active state carried by the brand hairline + fill only.
 */
export function PipelineStrip({ applications, active, onSelect }: PipelineStripProps) {
  const { t } = useTranslation();
  const counts = pipelineCounts(applications);
  const overdue = overdueCount(applications);

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
              onClick={() => onSelect(selected ? null : group.id)}
              className={cn(
                'surface-card flex min-h-11 flex-col items-start justify-center gap-1 px-3 py-2 text-left transition-colors',
                selected ? 'border-brand/45 bg-brand/[0.07]' : 'hover:bg-foreground/[0.03]'
              )}
            >
              <span className="truncate text-[10px] font-semibold uppercase tracking-wider text-foreground/45">
                {t(`applications.pipeline.${group.id}` as const)}
              </span>
              <span
                className={cn(
                  'text-lg font-semibold leading-none',
                  selected ? 'text-brand-soft' : 'text-foreground/85'
                )}
              >
                {counts[group.id]}
              </span>
            </Button>
          );
        })}
      </div>

      {overdue > 0 && (
        <div className="mt-2 flex justify-end">
          <Tag color="error" icon={<AlarmClock size={10} />} className="rounded-full text-[10px]">
            {t('applications.pipeline.overdue', { n: overdue })}
          </Tag>
        </div>
      )}
    </div>
  );
}
