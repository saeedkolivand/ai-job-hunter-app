import { Activity, CheckCircle, Cpu, Database, Loader2, RefreshCw, XCircle } from 'lucide-react';

import { Button, cn, GlassCard } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { useSystemHealth, useSystemMetrics } from '@/services';
import { invalidateHealth } from '@/services/use-system';

export function AISystemStatus() {
  const { t } = useTranslation();
  const { data: health, isFetching } = useSystemHealth();
  const { data: metricsRaw } = useSystemMetrics();

  type Health = {
    ai?: { ready: boolean; model?: string; memoryMB?: number };
    data?: { ready: boolean; sqlite: boolean; vector: boolean };
    workers?: { active: number; idle: number; max: number };
  };
  type Metrics = { processes?: Array<{ type: string; memory?: { workingSetSize: number } }> };

  const h = health as Health | undefined;
  const metrics = metricsRaw as Metrics | undefined;

  const rendererMem = metrics?.processes?.find((p) => p.type === 'renderer')?.memory
    ?.workingSetSize;

  const rows = [
    {
      name: t('dashboard.status.aiModel'),
      icon: Cpu,
      status: h == null ? 'loading' : h.ai?.ready ? 'ready' : 'error',
      detail: h?.ai?.ready
        ? (h.ai.model ?? 'Ready')
        : h != null
          ? t('dashboard.status.notAvailable')
          : t('dashboard.status.checking'),
    },
    {
      name: t('dashboard.status.database'),
      icon: Database,
      status: h == null ? 'loading' : h.data?.sqlite ? 'ready' : 'error',
      detail: h?.data?.sqlite
        ? t('dashboard.status.connected')
        : h != null
          ? t('dashboard.status.notAvailable')
          : t('dashboard.status.checking'),
    },
    {
      name: t('dashboard.status.vectorDb'),
      icon: Database,
      status: h == null ? 'loading' : h.data?.vector ? 'ready' : 'error',
      detail: h?.data?.vector
        ? t('dashboard.status.indexed')
        : h != null
          ? t('dashboard.status.notAvailable')
          : t('dashboard.status.checking'),
    },
    {
      name: t('dashboard.status.workers'),
      icon: Activity,
      status:
        h == null
          ? 'loading'
          : h.workers && h.workers.active + h.workers.idle > 0
            ? 'ready'
            : 'error',
      detail: h?.workers
        ? `${h.workers.active} active · ${h.workers.idle} idle`
        : h != null
          ? t('dashboard.status.notAvailable')
          : t('dashboard.status.checking'),
    },
  ] as const;

  const statusConfig = {
    ready: {
      icon: CheckCircle,
      color: 'text-emerald-400',
      bg: 'bg-emerald-500/15',
      dot: 'bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.5)]',
    },
    loading: {
      icon: Loader2,
      color: 'text-amber-400',
      bg: 'bg-amber-500/15',
      dot: 'bg-amber-400 animate-pulse',
    },
    error: { icon: XCircle, color: 'text-red-400', bg: 'bg-red-500/15', dot: 'bg-red-400' },
  };

  return (
    <GlassCard>
      <div className="mb-4 flex items-center justify-between">
        <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
          <Cpu size={14} />
          {t('dashboard.aiSystemStatus')}
        </div>
        <Button
          onClick={() => void invalidateHealth()}
          disabled={isFetching}
          className="flex items-center gap-1 rounded-lg bg-white/5 px-2 py-1 text-[10px] text-foreground/40 hover:text-foreground/70 h-auto border-transparent"
        >
          <RefreshCw size={10} className={isFetching ? 'animate-spin' : ''} />
          {t('dashboard.status.refresh')}
        </Button>
      </div>

      <div className="space-y-2">
        {rows.map((row) => {
          const cfg = statusConfig[row.status];
          const StatusIcon = cfg.icon;
          const RowIcon = row.icon;
          return (
            <div
              key={row.name}
              className="flex items-center gap-3 rounded-lg border border-white/[0.04] bg-white/[0.02] px-3 py-2.5"
            >
              <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-white/[0.04]">
                <RowIcon size={13} className="text-foreground/35" />
              </div>
              <div className="min-w-0 flex-1">
                <div className="text-xs font-medium text-foreground/80">{row.name}</div>
                <div className="text-[11px] text-foreground/40">{row.detail}</div>
              </div>
              <div
                className={cn(
                  'flex h-6 w-6 shrink-0 items-center justify-center rounded-full',
                  cfg.bg
                )}
              >
                <StatusIcon
                  size={13}
                  className={cn(cfg.color, row.status === 'loading' && 'animate-spin')}
                />
              </div>
            </div>
          );
        })}
      </div>

      {rendererMem != null && (
        <div className="mt-3 flex items-center justify-between rounded-lg border border-white/[0.04] bg-white/[0.02] px-3 py-2">
          <span className="text-[11px] text-foreground/35">{t('dashboard.status.memory')}</span>
          <span className="font-mono text-[11px] text-foreground/50">
            {Math.round(rendererMem / 1024)} MB
          </span>
        </div>
      )}
    </GlassCard>
  );
}
