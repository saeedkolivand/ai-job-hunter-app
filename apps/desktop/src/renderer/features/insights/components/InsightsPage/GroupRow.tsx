import { motion } from 'motion/react';

import type { InsightsOutcomeGroup } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { transition } from '@ajh/ui';

/** One slice of one dimension: label, positive-reply bar (when a rate is allowed) and counts. */
export function GroupRow({
  group,
  label,
  hint,
}: {
  group: InsightsOutcomeGroup;
  label: string;
  hint?: string | null;
}) {
  const { t } = useTranslation();
  const { counts, rates } = group;
  const percent = rates ? Math.round(rates.positiveReply * 100) : null;
  return (
    <li className="space-y-1.5">
      <div className="flex items-baseline justify-between gap-3 text-[12px]">
        <span className="min-w-0 truncate text-foreground/80">{label}</span>
        {percent !== null && (
          <span className="shrink-0 font-medium tabular-nums text-foreground/80">
            {t('insights.rate', { percent })}
          </span>
        )}
      </div>
      {percent !== null ? (
        <div
          role="meter"
          aria-label={label}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={percent}
          className="h-1.5 w-full overflow-hidden rounded-full bg-foreground/10"
        >
          <motion.div
            initial={false}
            animate={{ width: `${percent}%` }}
            transition={transition.normal}
            className="h-full rounded-full bg-brand"
          />
        </div>
      ) : (
        <p className="text-[12px] text-foreground/70">{t('insights.notEnoughData')}</p>
      )}
      <p className="text-[12px] tabular-nums text-foreground/70">
        {t('insights.counts', {
          positive: counts.positiveReply,
          rejected: counts.rejected,
          noReply: counts.noReply,
          pending: counts.pending,
        })}
      </p>
      {hint && <p className="text-[12px] text-foreground/70">{hint}</p>}
    </li>
  );
}
