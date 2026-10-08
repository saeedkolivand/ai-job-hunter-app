import { Bookmark, Briefcase, CheckCircle, Eye, TrendingUp } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { GlassCard } from '@ajh/ui';

import { pipelineCounts } from '@/lib/application-pipeline';
import { useApplications, useInteractions } from '@/services';

export function JobPipelineOverview() {
  const { t } = useTranslation();

  // Saved / Applied / Total come from the `applications` table — the same
  // source and `pipelineCounts` the Applications page uses — so the two never
  // disagree (extension imports write applications only). "Viewed" has no
  // application stage, so it stays on the interactions log.
  const { data: applications = [] } = useApplications();
  const { data: viewed = [] } = useInteractions('viewed');
  const counts = pipelineCounts(applications);

  const stats = [
    {
      label: t('dashboard.savedJobs'),
      value: counts.saved,
      icon: Bookmark,
      color: 'text-blue-400',
      bg: 'bg-blue-400/10',
    },
    {
      label: t('dashboard.applied'),
      value: counts.applied,
      icon: CheckCircle,
      color: 'text-emerald-400',
      bg: 'bg-emerald-400/10',
    },
    {
      label: t('dashboard.viewed'),
      value: (viewed as unknown[]).length,
      icon: Eye,
      color: 'text-orange-400',
      bg: 'bg-orange-400/10',
    },
    {
      label: t('dashboard.totalTracked'),
      value: applications.length,
      icon: TrendingUp,
      color: 'text-purple-400',
      bg: 'bg-purple-400/10',
    },
  ];

  return (
    <GlassCard>
      <div className="mb-4 flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
        <Briefcase size={14} />
        {t('dashboard.jobPipeline')}
      </div>

      <div className="grid grid-cols-1 gap-3 @xs:grid-cols-2">
        {stats.map((stat) => {
          const Icon = stat.icon;
          return (
            <div
              key={stat.label}
              className="flex flex-col items-center gap-2 rounded-xl border border-foreground/[0.06] bg-foreground/[0.03] px-3 py-3.5"
            >
              <div className={`flex h-8 w-8 items-center justify-center rounded-lg ${stat.bg}`}>
                <Icon size={15} className={stat.color} />
              </div>
              <div className="text-3xl font-bold tabular-nums text-foreground">{stat.value}</div>
              <div className="text-center text-[11px] text-muted-foreground">{stat.label}</div>
            </div>
          );
        })}
      </div>

      {applications.length === 0 && (
        <p className="mt-3 text-center text-xs text-muted-foreground">
          {t('dashboard.noJobsTracked')}
        </p>
      )}
    </GlassCard>
  );
}
