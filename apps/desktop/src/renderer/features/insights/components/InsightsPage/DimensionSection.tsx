import { useId } from 'react';

import type { InsightsDimension, InsightsOutcomeGroup } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { GlassCard } from '@ajh/ui';

import { groupHint, groupLabel } from '../../lib/group-label';
import { GroupRow } from './GroupRow';

export function DimensionSection({
  dimension,
  groups,
}: {
  dimension: InsightsDimension;
  groups: InsightsOutcomeGroup[];
}) {
  const { t, i18n } = useTranslation();
  const headingId = useId();
  return (
    <GlassCard className="p-5">
      <section aria-labelledby={headingId} className="space-y-4">
        <h2 id={headingId} className="text-sm font-semibold text-foreground/85">
          {t(`insights.dimension.${dimension}`)}
        </h2>
        <ul className="space-y-4">
          {groups.map((group) => (
            <GroupRow
              key={group.key}
              group={group}
              label={groupLabel(t, i18n.language, dimension, group.key)}
              hint={groupHint(t, dimension, group.key)}
            />
          ))}
        </ul>
      </section>
    </GlassCard>
  );
}
