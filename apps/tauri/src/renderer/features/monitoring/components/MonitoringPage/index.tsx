import { BarChart3, CheckCircle2, Cpu, Loader2, XCircle, Zap } from 'lucide-react';
import { useMemo } from 'react';

import { GlassCard } from '@ajh/ui';

import { PageHeader } from '@/components/layout/PageHeader';
import { PageTransition } from '@/components/layout/PageTransition';
import { ActiveJobsSection } from '@/features/monitoring/components/ActiveJobsSection';
import { ActivityFeedSection } from '@/features/monitoring/components/ActivityFeedSection';
import { MetricCard } from '@/features/monitoring/components/MetricCard';
import { Sparkline } from '@/features/monitoring/components/Sparkline';
import { StatusDot } from '@/features/monitoring/components/StatusDot';
import { useActivityFeed } from '@/features/monitoring/hooks/useActivityFeed';
import { useJobMetrics } from '@/features/monitoring/hooks/useJobMetrics';
import type { JobRecord } from '@/features/monitoring/types';
import { useKindLabelMap } from '@/hooks/use-kind-label-map';
import { useTranslation } from '@/lib/i18n';
import { useAppVersion, useJobQueue, useSystemHealth } from '@/services';

export function MonitoringPage() {
  const { t } = useTranslation();

  const KIND_LABEL_MAP = useKindLabelMap();

  const { data: healthData } = useSystemHealth();
  const health = (healthData ?? {}) as {
    ai?: { ready: boolean; model?: string };
    data?: { ready: boolean; sqlite: boolean; vector: boolean };
  };
  const { data: allJobsData } = useJobQueue();
  const { data: appVersionData } = useAppVersion();
  const appVersion = appVersionData ?? '';

  const allJobs = useMemo(() => (allJobsData ?? []) as JobRecord[], [allJobsData]);

  const { activeJobs, counters, successRate } = useJobMetrics(allJobs);
  const { activity } = useActivityFeed(allJobs, KIND_LABEL_MAP);

  const last24h = useMemo(() => {
    const bins = Array.from({ length: 24 }, () => 0);
    allJobs
      .filter((j) => j.status === 'completed')
      .forEach((j) => {
        const ts = j.finishedAt ?? j.updatedAt;
        const h = new Date(ts).getHours();
        if (h >= 0 && h < 24) bins[h] = (bins[h] ?? 0) + 1;
      });
    return bins;
  }, [allJobs]);

  const metrics = [
    {
      label: t('monitoring.metrics.completed'),
      value: counters.completed,
      icon: CheckCircle2,
      color: 'text-emerald-400',
      bg: 'bg-emerald-500/10',
    },
    {
      label: t('monitoring.metrics.running'),
      value: counters.running,
      icon: Loader2,
      color: 'text-blue-400',
      bg: 'bg-blue-500/10',
    },
    {
      label: t('monitoring.metrics.failed'),
      value: counters.failed,
      icon: XCircle,
      color: 'text-amber-400',
      bg: 'bg-amber-500/10',
    },
    {
      label: t('monitoring.metrics.successRate'),
      value: `${successRate}%`,
      icon: Zap,
      color: 'text-purple-400',
      bg: 'bg-purple-500/10',
    },
  ];

  return (
    <PageTransition className="h-full overflow-y-auto px-10 py-10">
      <div className="mx-auto max-w-5xl space-y-5">
        <PageHeader
          title={t('monitoring.title')}
          subtitle={t('monitoring.subtitle')}
          badge={t('monitoring.badge')}
          actions={
            <div className="flex items-center gap-4 text-[11px]">
              <StatusDot
                label={t('monitoring.status.ollama')}
                ready={health.ai?.ready ?? false}
                detail={health.ai?.model}
              />
              <StatusDot
                label={t('monitoring.status.database')}
                ready={health.data?.ready ?? false}
              />
              {appVersion && <span className="text-foreground/30">v{appVersion}</span>}
            </div>
          }
        />

        <div className="grid grid-cols-4 gap-3">
          {metrics.map(({ label, value, icon: Icon, color, bg }) => (
            <MetricCard
              key={label}
              label={label}
              value={value}
              icon={Icon}
              color={color}
              bg={bg}
              animate={label === 'Running' && counters.running > 0}
            />
          ))}
        </div>

        <div className="grid grid-cols-5 gap-4">
          <ActiveJobsSection activeJobs={activeJobs} kindLabel={KIND_LABEL_MAP} t={t} />
          <ActivityFeedSection activity={activity} t={t} />
        </div>

        <GlassCard>
          <div className="mb-3 flex items-center justify-between">
            <div className="flex items-center gap-2">
              <BarChart3 size={12} className="text-brand-soft" />
              <span className="text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
                {t('monitoring.sections.hourlyActivity')}
              </span>
            </div>
            <span className="text-[10px] text-foreground/30">
              {t('monitoring.timeLabels.last24Hours')}
            </span>
          </div>
          <Sparkline data={last24h} />
        </GlassCard>

        <div className="flex items-center justify-between rounded-xl border border-white/[0.05] px-4 py-3 text-[11px] text-foreground/35">
          <div className="flex items-center gap-2">
            <Cpu size={11} className="text-foreground/25" />
            {t('monitoring.footer.localProcessing')}
          </div>
          <span>{t('monitoring.footer.localPrivateOffline')}</span>
        </div>
      </div>
    </PageTransition>
  );
}
