import { Activity, ArrowRight, Cpu, Database, Loader2, Sparkles } from 'lucide-react';
import { useRouter } from '@tanstack/react-router';

import { Button, cn, HoverPopover } from '@ajh/ui';

import { ROUTES } from '@/constants/routes';
import { useKindLabelMap } from '@/hooks/use-kind-label-map';
import { useTranslation } from '@/lib/i18n';
import { useCapabilities } from '@/providers/CapabilityProvider';
import { useWorkerActivity } from '@/services';
import { useAIModel, useAiProviderConfig } from '@/store/preferences-store';

export function StatusBar() {
  const { t } = useTranslation();
  const router = useRouter();
  const { ai, data } = useCapabilities();
  const kindLabelMap = useKindLabelMap();
  const activity = useWorkerActivity(kindLabelMap);
  const aiModel = useAIModel();
  const providerConfig = useAiProviderConfig();

  // Get current model name from active provider
  const activeProvider = providerConfig?.activeProvider ?? 'ollama';
  const currentModel =
    activeProvider === 'ollama'
      ? aiModel?.defaultModel
      : providerConfig?.providers?.[activeProvider]?.model;

  const statusColor = () => {
    if (!ai.ready) return 'text-red-400';
    if (currentModel) return 'text-emerald-400';
    return 'text-amber-400/80';
  };

  const statusText = () => {
    if (currentModel) return currentModel;
    if (!ai.ready) return t('status.ollamaOffline');
    return t('status.ready');
  };

  const dbStatus = () => {
    if (!data.sqlite && !data.vector) return 'Offline';
    if (data.sqlite && data.vector) return 'SQLite · LanceDB';
    if (data.sqlite) return 'SQLite';
    if (data.vector) return 'LanceDB';
    return 'Partial';
  };

  const activityText = () =>
    activity.isActive
      ? `${activity.active} ${t('status.running')} · ${activity.queued} ${t('status.queued')}`
      : t('status.idle');

  const goMonitoring = () => {
    void router.navigate({ to: ROUTES.MONITORING });
  };

  return (
    <div className="glass-surface mx-3 mb-3 mt-2 flex items-center justify-between rounded-xl px-4 py-1.5 text-[11px] text-foreground/60">
      <div className="flex items-center gap-4">
        <span className="flex items-center gap-1.5">
          <Sparkles size={12} className={statusColor()} />
          {statusText()}
        </span>
        <span className="flex items-center gap-1.5">
          <Database size={12} className={data.ready ? 'text-emerald-400' : 'text-foreground/40'} />
          {dbStatus()}
        </span>

        {/* Activity indicator + upward popover (bar is at screen bottom) */}
        <HoverPopover
          placement="top"
          ariaLabel={t('status.activity')}
          trigger={
            <Button
              variant="unstyled"
              aria-label={t('status.activity')}
              className="flex items-center gap-1.5 rounded"
              onClick={goMonitoring}
            >
              <Activity
                size={12}
                className={cn(
                  activity.isActive ? 'text-emerald-400' : 'text-foreground/40',
                  activity.isActive && 'animate-pulse'
                )}
              />
              {activityText()}
            </Button>
          }
          contentClassName="w-64 rounded-xl border border-white/[0.1] bg-black/95 p-3 shadow-2xl"
        >
          {activity.isActive || activity.queued > 0 ? (
            <div className="space-y-1.5">
              {activity.running.map((job) => (
                <div key={job.id} className="flex items-center gap-2">
                  <Loader2 size={11} className="shrink-0 animate-spin text-emerald-400" />
                  <span className="flex-1 truncate text-xs text-foreground/80">
                    {kindLabelMap[job.kind] ?? job.kind}
                  </span>
                  {job.progress > 0 && job.progress < 1 && (
                    <span className="font-mono text-[10px] text-foreground/40">
                      {Math.round(job.progress * 100)}%
                    </span>
                  )}
                </div>
              ))}
              {activity.queuedJobs.map((job) => (
                <div key={job.id} className="flex items-center gap-2 opacity-60">
                  <Cpu size={11} className="shrink-0 text-foreground/40" />
                  <span className="flex-1 truncate text-xs text-foreground/60">
                    {kindLabelMap[job.kind] ?? job.kind}
                  </span>
                  <span className="text-[10px] text-foreground/30">{t('status.queued')}</span>
                </div>
              ))}
            </div>
          ) : (
            <div className="text-xs text-foreground/50">{t('status.activityIdle')}</div>
          )}
          <Button
            variant="unstyled"
            onClick={goMonitoring}
            className="mt-3 flex w-full items-center gap-1.5 rounded-lg border border-white/[0.06] bg-white/[0.03] px-2 py-1.5 text-left text-[11px] text-foreground/60 transition-colors hover:text-foreground/90"
          >
            {t('status.viewAll')}
            <ArrowRight size={11} />
          </Button>
        </HoverPopover>
      </div>
      <div className="text-foreground/40">local-first · offline-capable</div>
    </div>
  );
}
