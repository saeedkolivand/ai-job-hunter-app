import { Activity, CheckCircle, Cpu, Database, Loader2, RefreshCw, XCircle } from 'lucide-react';
import { useState } from 'react';
import { useRouter } from '@tanstack/react-router';

import { useTranslation } from '@ajh/translations';
import { Button, cn, GlassCard } from '@ajh/ui';

import { useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
import { ROUTES } from '@/constants/routes';
import { useKindLabelMap } from '@/hooks/use-kind-label-map';
import { useSystemHealth, useSystemMetrics, useWorkerActivity } from '@/services';
import { keys, queryClient } from '@/services/query-client';
import { invalidateHealth } from '@/services/use-system';

/** Mirrors `AiSetupHint`'s `MESSAGE_KEY` — the copy for each `useCanUseAI` block
 *  reason. Duplicated rather than imported: every other gating call site
 *  (AnalyzeLeftPanel, LeftPanel, ReferralModal, StepModel…) inlines its own
 *  copy of this tiny reason→key map instead of sharing one. */
const AI_REASON_KEY: Record<string, string> = {
  addApiKey: 'aiSetup.addApiKey',
  selectModel: 'aiSetup.selectModel',
  installCli: 'aiSetup.installCli',
};

export function AISystemStatus() {
  const { t } = useTranslation();
  const router = useRouter();
  const { data: health, isFetching } = useSystemHealth();
  const { data: metricsRaw } = useSystemMetrics();
  const activity = useWorkerActivity(useKindLabelMap());
  const [refreshing, setRefreshing] = useState(false);
  // `system_health.ai.ready` (below, `h.ai`) only ever probes the LOCAL Ollama
  // daemon — regardless of which provider is actually active — so a user on
  // OpenAI/Anthropic/Gemini/a CLI agent (with local Ollama not even running)
  // saw a false "AI Model not available" here. `useCanUseAI` is the
  // provider-kind-aware source of truth already gating every other AI action
  // in the app; reuse it for this row instead of the Ollama-only probe.
  const { canUse: aiCanUse, reason: aiReason } = useCanUseAI();
  const selectedModel = useSelectedModel();

  // `ai` is deliberately absent: this row now reads `useCanUseAI` (above),
  // not the Ollama-only `system_health.ai` probe.
  type Health = {
    data?: { ready: boolean; sqlite: boolean; vector: boolean };
  };
  type Metrics = { processes?: Array<{ type: string; memory?: { workingSetSize: number } }> };

  const h = health as Health | undefined;
  const metrics = metricsRaw as Metrics | undefined;

  const rendererMem = metrics?.processes?.find((p) => p.type === 'renderer')?.memory
    ?.workingSetSize;

  const refresh = async () => {
    setRefreshing(true);
    try {
      await Promise.all([
        invalidateHealth(),
        queryClient.invalidateQueries({ queryKey: keys.jobs.all }),
        queryClient.invalidateQueries({ queryKey: keys.system.metrics }),
      ]);
    } finally {
      setRefreshing(false);
    }
  };

  const rows = [
    {
      name: t('dashboard.status.aiModel'),
      icon: Cpu,
      // `aiReason` is `undefined` while `useActiveConfig` is still on its
      // first (boot-prefetched) fetch — see `useCanUseAI`'s own "cold boot"
      // branch — so that, not `h`, is this row's loading signal now.
      status: aiCanUse ? 'ready' : aiReason == null ? 'loading' : 'error',
      detail: aiCanUse
        ? selectedModel || t('dashboard.status.ready')
        : aiReason
          ? t(AI_REASON_KEY[aiReason] ?? 'aiSetup.addApiKey')
          : t('dashboard.status.checking'),
      onClick: undefined as (() => void) | undefined,
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
      onClick: undefined,
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
      onClick: undefined,
    },
    {
      name: t('dashboard.status.activity'),
      icon: Activity,
      status: activity.isActive ? 'ready' : 'idle',
      detail: activity.isActive
        ? `${activity.active} ${t('status.running')} · ${activity.queued} ${t('status.queued')}`
        : t('status.idle'),
      onClick: () => void router.navigate({ to: ROUTES.MONITORING }),
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
    idle: {
      icon: CheckCircle,
      color: 'text-foreground/40',
      bg: 'bg-muted',
      dot: 'bg-foreground/30',
    },
    error: { icon: XCircle, color: 'text-red-400', bg: 'bg-red-500/15', dot: 'bg-red-400' },
  };

  return (
    <GlassCard>
      <div className="mb-4 flex items-center justify-between">
        <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
          <Cpu size={14} />
          {t('dashboard.aiSystemStatus')}
        </div>
        <Button
          onClick={() => void refresh()}
          disabled={isFetching || refreshing}
          className="flex items-center gap-1 rounded-lg bg-muted px-2 py-1 text-[10px] text-muted-foreground hover:text-foreground/70 h-auto border-transparent"
        >
          <RefreshCw size={10} className={isFetching || refreshing ? 'animate-spin' : ''} />
          {t('dashboard.status.refresh')}
        </Button>
      </div>

      <div className="space-y-2">
        {rows.map((row) => {
          const cfg = statusConfig[row.status];
          const StatusIcon = cfg.icon;
          const RowIcon = row.icon;
          const content = (
            <>
              <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-muted">
                <RowIcon size={13} className="text-foreground/35" />
              </div>
              <div className="min-w-0 flex-1 text-left">
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
            </>
          );
          const rowClass =
            'flex w-full items-center gap-3 rounded-lg border border-[var(--border-clear)] bg-card px-3 py-2.5';
          return row.onClick ? (
            <Button
              key={row.name}
              variant="unstyled"
              onClick={row.onClick}
              className={cn(rowClass, 'transition-colors hover:bg-muted')}
            >
              {content}
            </Button>
          ) : (
            <div key={row.name} className={rowClass}>
              {content}
            </div>
          );
        })}
      </div>

      {rendererMem != null && (
        <div className="mt-3 flex items-center justify-between rounded-lg border border-[var(--border-clear)] bg-card px-3 py-2">
          <span className="text-[11px] text-foreground/35">{t('dashboard.status.memory')}</span>
          <span className="font-mono text-[11px] text-foreground/50">
            {Math.round(rendererMem / 1024)} MB
          </span>
        </div>
      )}
    </GlassCard>
  );
}
