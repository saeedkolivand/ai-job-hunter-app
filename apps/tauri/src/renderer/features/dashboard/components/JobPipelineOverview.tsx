import { Briefcase, CheckCircle, Clock, type LucideIcon, TrendingUp } from 'lucide-react';

import { GlassCard } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

interface PipelineStat {
  label: string;
  labelKey?: string;
  value: string | number;
  icon: LucideIcon;
  color: string;
  trend?: string;
}

const PIPELINE_STATS: PipelineStat[] = [
  {
    label: 'Saved Jobs',
    labelKey: 'dashboard.savedJobs',
    value: 24,
    icon: Briefcase,
    color: 'text-blue-400',
    trend: '+3 this week',
  },
  {
    label: 'Applied',
    labelKey: 'dashboard.applied',
    value: 12,
    icon: CheckCircle,
    color: 'text-green-400',
    trend: '+2 this week',
  },
  {
    label: 'Interviews',
    labelKey: 'dashboard.interviews',
    value: 3,
    icon: Clock,
    color: 'text-orange-400',
    trend: '+1 this week',
  },
  {
    label: 'AI Matches',
    labelKey: 'dashboard.aiMatches',
    value: 45,
    icon: TrendingUp,
    color: 'text-purple-400',
    trend: '+12 this week',
  },
];

export function JobPipelineOverview() {
  const { t } = useTranslation();

  return (
    <GlassCard>
      <div className="mb-4 flex items-center justify-between">
        <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
          <Briefcase size={14} />
          {t('dashboard.jobPipeline')}
        </div>
      </div>

      <div className="grid grid-cols-2 gap-3">
        {PIPELINE_STATS.map((stat) => {
          const Icon = stat.icon;
          return (
            <div
              key={stat.label}
              className="flex flex-col items-center gap-2 rounded-lg bg-white/5 px-3 py-3"
            >
              <Icon size={20} className={stat.color} />
              <div className="text-2xl font-semibold text-foreground">{stat.value}</div>
              <div className="text-xs text-foreground/40 text-center">
                {stat.labelKey ? t(stat.labelKey) : stat.label}
              </div>
              {stat.trend && <div className="text-[10px] text-foreground/30">{stat.trend}</div>}
            </div>
          );
        })}
      </div>
    </GlassCard>
  );
}
