import { useRouter } from '@tanstack/react-router';

import { ActionTile } from '@ajh/ui';

import { PageHeader } from '@/components/layout/PageHeader';
import { AISystemStatus } from '@/features/dashboard/components/AISystemStatus';
import { JobPipelineOverview } from '@/features/dashboard/components/JobPipelineOverview';
import { useTranslation } from '@/lib/i18n';
import { useUserName } from '@/store/preferences-store';

import { QUICK_ACTIONS } from '../../constants';

function Dashboard() {
  const { t } = useTranslation();
  const router = useRouter();
  const userName = useUserName();

  return (
    <div
      className="h-full overflow-y-auto px-10 py-10"
      style={{ '--stagger-base': '60ms' } as React.CSSProperties}
    >
      <div className="mx-auto max-w-6xl">
        <PageHeader
          title={userName ? `${t('dashboard.welcome')}, ${userName}` : t('dashboard.welcome')}
          subtitle={t('dashboard.subtitle')}
        />

        {/* Quick Actions */}
        <div className="mb-8 grid grid-cols-2 gap-4 md:grid-cols-4">
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
        <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
          <div className="md:col-span-2 lg:col-span-2">
            <JobPipelineOverview />
          </div>
          <div className="lg:col-span-1">
            <AISystemStatus />
          </div>
        </div>
      </div>
    </div>
  );
}

export { Dashboard };
