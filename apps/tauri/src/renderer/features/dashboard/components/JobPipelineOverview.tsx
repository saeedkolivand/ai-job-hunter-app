import { Bookmark, Briefcase, CheckCircle, Eye, TrendingUp } from 'lucide-react';

import { GlassCard } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { useInteractions } from '@/services';

export function JobPipelineOverview() {
  const { t } = useTranslation();

  const { data: bookmarked = [] } = useInteractions('bookmarked');
  const { data: applied = [] } = useInteractions('applied');
  const { data: viewed = [] } = useInteractions('viewed');
  const { data: allInteractions = [] } = useInteractions();

  const stats = [
    {
      label: t('dashboard.savedJobs'),
      value: (bookmarked as unknown[]).length,
      icon: Bookmark,
      color: 'text-blue-400',
      bg: 'bg-blue-400/10',
    },
    {
      label: t('dashboard.applied'),
      value: (applied as unknown[]).length,
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
      value: (allInteractions as unknown[]).length,
      icon: TrendingUp,
      color: 'text-purple-400',
      bg: 'bg-purple-400/10',
    },
  ];

  return (
    <GlassCard>
      <div className="mb-4 flex items-center gap-2 text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
        <Briefcase size={14} />
        {t('dashboard.jobPipeline')}
      </div>

      <div className="grid grid-cols-2 gap-3">
        {stats.map((stat) => {
          const Icon = stat.icon;
          return (
            <div
              key={stat.label}
              className="flex flex-col items-center gap-2 rounded-xl border border-white/[0.05] bg-white/[0.03] px-3 py-3.5"
            >
              <div className={`flex h-8 w-8 items-center justify-center rounded-lg ${stat.bg}`}>
                <Icon size={15} className={stat.color} />
              </div>
              <div className="text-2xl font-semibold tabular-nums text-foreground">
                {stat.value}
              </div>
              <div className="text-center text-[11px] text-foreground/40">{stat.label}</div>
            </div>
          );
        })}
      </div>

      {(allInteractions as unknown[]).length === 0 && (
        <p className="mt-3 text-center text-xs text-foreground/30">
          {t('dashboard.noJobsTracked')}
        </p>
      )}
    </GlassCard>
  );
}
