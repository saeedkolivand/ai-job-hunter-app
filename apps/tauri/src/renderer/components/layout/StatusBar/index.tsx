import { Cpu, Database, Sparkles } from 'lucide-react';

import { useTranslation } from '@/lib/i18n';
import { useCapabilities } from '@/providers/CapabilityProvider';
import { useAIModel, useAiProviderConfig } from '@/store/preferences-store';

export function StatusBar() {
  const { t } = useTranslation();
  const { ai, data, workers } = useCapabilities();
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

  const workerStatus = () => {
    if (workers.active > 0) return `${workers.active} active`;
    if (workers.idle > 0) return `${workers.idle} idle`;
    return 'Idle';
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
        <span className="flex items-center gap-1.5">
          <Cpu
            size={12}
            className={workers.active > 0 ? 'text-emerald-400' : 'text-foreground/40'}
          />
          {workerStatus()}
        </span>
      </div>
      <div className="text-foreground/40">local-first · offline-capable</div>
    </div>
  );
}
