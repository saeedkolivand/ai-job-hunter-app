import { TrendingUp } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { CardSkeleton, EmptyState, ErrorState, RefreshButton } from '@ajh/ui';

import { PageShell } from '@/components/layout/PageShell';
import { useFormatRelativeTime } from '@/hooks/use-format-relative-time';
import { useInsightsOutcomes } from '@/services';

import { DIMENSIONS, groupsOf, headlineOf } from '../../lib/insights';
import { DimensionSection } from './DimensionSection';
import { Headline } from './Headline';
import { VolumeNudge } from './VolumeNudge';

function InsightsPage() {
  const { t } = useTranslation();
  const formatRelative = useFormatRelativeTime(t, 'resumes.relativeTime');
  const { data, isLoading, isError, refetch } = useInsightsOutcomes();
  const refresh = () => void refetch();

  return (
    <PageShell title={t('insights.title')} subtitle={t('insights.subtitle')}>
      {data ? (
        <div className="space-y-4">
          <div className="flex items-center justify-between gap-3 text-[12px] text-foreground/70">
            <span>{t('insights.updated', { time: formatRelative(data.generatedAt) })}</span>
            <RefreshButton onRefresh={refresh}>{t('insights.refresh')}</RefreshButton>
          </div>
          {isError && (
            <p role="status" className="text-[12px] text-foreground/80">
              {t('insights.stale')}
            </p>
          )}
          {data.nudge && <VolumeNudge nudge={data.nudge} />}
          {data.groups.length === 0 ? (
            <EmptyState
              icon={TrendingUp}
              title={t('insights.empty.title')}
              description={t('insights.empty.description')}
            />
          ) : (
            <>
              <Headline headline={headlineOf(data.groups)} />
              {DIMENSIONS.map((dimension) => {
                const groups = groupsOf(data.groups, dimension);
                return groups.length > 0 ? (
                  <DimensionSection key={dimension} dimension={dimension} groups={groups} />
                ) : null;
              })}
            </>
          )}
        </div>
      ) : isLoading ? (
        <div className="space-y-3">
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
        </div>
      ) : (
        <ErrorState
          title={t('insights.errorTitle')}
          description={t('insights.errorDescription')}
          onRetry={refresh}
        />
      )}
    </PageShell>
  );
}

export { InsightsPage };
