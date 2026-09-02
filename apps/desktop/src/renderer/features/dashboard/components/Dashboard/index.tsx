import { useRouter } from '@tanstack/react-router';

import { useTranslation } from '@ajh/translations';
import { ActionTile } from '@ajh/ui';

import { PageHeader } from '@/components/layout/PageHeader';
import { AISystemStatus } from '@/features/dashboard/components/AISystemStatus';
import { JobPipelineOverview } from '@/features/dashboard/components/JobPipelineOverview';
import { NextStepTile } from '@/features/dashboard/components/NextStepTile';
import { QUICK_ACTIONS } from '@/features/dashboard/constants';
import { useUserName } from '@/store/preferences-store';

function Dashboard() {
  const { t } = useTranslation();
  const router = useRouter();
  const userName = useUserName();

  return (
    <div
      className="h-full overflow-y-auto px-10 py-10"
      style={{ '--stagger-base': '60ms' } as React.CSSProperties}
    >
      <div className="@container mx-auto max-w-6xl 2xl:max-w-7xl">
        <PageHeader
          title={userName ? `${t('dashboard.welcome')}, ${userName}` : t('dashboard.welcome')}
          subtitle={t('dashboard.subtitle')}
        />

        {/* The one thing to do next — full width, not a fifth quick action:
            the grid below is 4-up and its stagger classes only go to 4. */}
        <NextStepTile />

        {/* Quick Actions */}
        <div className="mb-8 grid grid-cols-2 gap-4 @2xl:grid-cols-4">
          {QUICK_ACTIONS.map(({ icon, labelKey, path }, i) => (
            <div key={path} className={`animate-slide-up stagger-${(i + 1) as 1 | 2 | 3 | 4}`}>
              <ActionTile
                icon={icon}
                label={t(labelKey)}
                onClick={() => router.navigate({ to: path })}
              />
            </div>
          ))}
        </div>

        {/* Dashboard Grid */}
        <div className="grid gap-6 @xl:grid-cols-2 @4xl:grid-cols-3">
          <div className="@xl:col-span-2">
            <JobPipelineOverview />
          </div>
          <div>
            <AISystemStatus />
          </div>
        </div>
      </div>
    </div>
  );
}

export { Dashboard };
