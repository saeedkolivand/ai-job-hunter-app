import { TrendingUp } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { CardSkeleton, EmptyState, ErrorState } from '@ajh/ui';

import { PageShell } from '@/components/layout/PageShell';
import { useInsightsOutcomes } from '@/services';

import { DIMENSIONS, groupsOf, headlineOf } from '../../lib/insights';
import { DimensionSection } from './DimensionSection';
import { Headline } from './Headline';
import { VolumeNudge } from './VolumeNudge';

function InsightsPage() {
  const { t } = useTranslation();
  const { data, isLoading, isError, refetch } = useInsightsOutcomes();

  return (
    <PageShell title={t('insights.title')} subtitle={t('insights.subtitle')}>
      {isLoading ? (
        <div className="space-y-3">
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
        </div>
      ) : isError || !data ? (
        <ErrorState
          title={t('insights.errorTitle')}
          description={t('insights.errorDescription')}
          onRetry={() => void refetch()}
        />
      ) : data.groups.length === 0 ? (
        <EmptyState
          icon={TrendingUp}
          title={t('insights.empty.title')}
          description={t('insights.empty.description')}
        />
      ) : (
        <div className="space-y-4">
          {data.nudge && <VolumeNudge nudge={data.nudge} />}
          <Headline headline={headlineOf(data.groups)} />
          {DIMENSIONS.map((dimension) => {
            const groups = groupsOf(data.groups, dimension);
            return groups.length > 0 ? (
              <DimensionSection key={dimension} dimension={dimension} groups={groups} />
            ) : null;
          })}
        </div>
      )}
    </PageShell>
  );
}

export { InsightsPage };
