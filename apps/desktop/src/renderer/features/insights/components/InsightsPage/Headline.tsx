import { useTranslation } from '@ajh/translations';
import { GlassCard } from '@ajh/ui';

import type { Headline as HeadlineData } from '../../lib/insights';

/** Overall positive-reply rate (only when there is enough data) and the raw counts. */
export function Headline({ headline }: { headline: HeadlineData }) {
  const { t } = useTranslation();
  const { counts, decided, rate } = headline;
  return (
    <GlassCard className="space-y-2 p-5">
      <p className="text-2xl font-semibold tabular-nums text-foreground/90">
        {rate === null
          ? t('insights.headline.noRate')
          : t('insights.headline.rate', { percent: Math.round(rate * 100) })}
      </p>
      <p className="text-[12px] tabular-nums text-foreground/65">
        {t('insights.headline.counts', {
          positive: counts.positiveReply,
          decided,
          pending: counts.pending,
        })}
      </p>
      <p className="text-[11px] text-foreground/65">{t('insights.headline.definition')}</p>
    </GlassCard>
  );
}
