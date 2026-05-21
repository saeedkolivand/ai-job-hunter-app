import { CheckCircle2, Download, ExternalLink, Loader2, RefreshCw, WifiOff } from 'lucide-react';
import { motion } from 'motion/react';
import { useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import { Button, GlassCard, useToast } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';
import { useAIModels, useOpenExternal, usePullModel, useSystemHealth } from '@/services';
import { keys } from '@/services/query-client';
import { useAIModel, usePreferencesStore } from '@/store/preferences-store';
import type { Model } from '@/types';

import { CustomDropdown } from './CustomDropdown';

const QUICK_MODELS = ['llama3.2', 'mistral', 'llama3.1:8b', 'llama3.2:1b'];

export function AISettingsTab() {
  const { t } = useTranslation();
  const toast = useToast();
  const qc = useQueryClient();
  const { data: modelList = [], isFetching: loadingModels } = useAIModels();
  const models = modelList as Model[];
  const aiModel = useAIModel();
  const setAIModel = usePreferencesStore((state) => state.setAIModel);
  const { data: health } = useSystemHealth();
  const ollamaReady = (health as { ai?: { ready: boolean } } | undefined)?.ai?.ready ?? false;
  const pullModel = usePullModel();
  const openExternal = useOpenExternal();
  const [pulling, setPulling] = useState<string | null>(null);

  const selectedModel = aiModel?.defaultModel || '';

  const handleSelectModel = (modelName: string) => {
    setAIModel({
      defaultModel: modelName,
      temperature: aiModel?.temperature || 0.7,
      maxTokens: aiModel?.maxTokens || 2048,
    });
  };

  const handlePull = async (model: string) => {
    setPulling(model);
    try {
      await pullModel.mutateAsync(model);
      qc.invalidateQueries({ queryKey: keys.ai.models });
      handleSelectModel(model);
      toast(`${model} downloaded and selected.`, 'success');
    } catch (err) {
      toast(err instanceof Error ? err.message : 'Download failed.', 'error');
    } finally {
      setPulling(null);
    }
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={transition.normal}
      className="space-y-4"
    >
      {/* Ollama status */}
      <GlassCard>
        <div className="mb-3 text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
          Ollama
        </div>
        {ollamaReady ? (
          <div className="flex items-center gap-2 text-sm text-emerald-400">
            <CheckCircle2 size={14} />
            {t('onboarding.ollama.ready')}
          </div>
        ) : (
          <div className="space-y-3">
            <div className="flex items-center gap-2 text-sm text-amber-400/80">
              <WifiOff size={14} />
              {t('onboarding.ollama.notFound')}
            </div>
            <div className="flex gap-2">
              <Button
                variant="ghost"
                size="sm"
                className="flex items-center gap-1.5 text-foreground/50"
                onClick={() => void openExternal.mutateAsync('https://ollama.com')}
              >
                <ExternalLink size={12} />
                {t('onboarding.ollama.downloadButton')}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                className="flex items-center gap-1.5 text-foreground/40"
                onClick={() => {
                  qc.invalidateQueries({ queryKey: keys.system.health });
                  qc.invalidateQueries({ queryKey: keys.ai.models });
                }}
              >
                <Loader2 size={12} />
                {t('onboarding.ollama.recheck')}
              </Button>
            </div>
          </div>
        )}
      </GlassCard>

      {/* Model selection */}
      <GlassCard>
        <div className="mb-3 flex items-center justify-between">
          <div className="text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
            {t('settings.aiModel.title')}
          </div>
          <Button
            onClick={() => void qc.invalidateQueries({ queryKey: keys.ai.models })}
            disabled={loadingModels}
            className="flex items-center gap-1.5 rounded-lg bg-white/5 px-3 py-1.5 text-xs text-foreground/70 hover:text-foreground transition-colors disabled:opacity-50 h-auto border-transparent"
          >
            <RefreshCw size={12} className={loadingModels ? 'animate-spin' : ''} />
            {t('settings.aiModel.refresh')}
          </Button>
        </div>
        <p className="mb-4 text-sm text-foreground/55">{t('settings.aiModel.description')}</p>

        {loadingModels ? (
          <div className="text-sm text-foreground/50">{t('settings.aiModel.loading')}</div>
        ) : models.length === 0 ? (
          <div className="space-y-3">
            <p className="text-sm text-foreground/50">{t('settings.aiModel.noModels')}</p>
            {ollamaReady && (
              <div className="space-y-2">
                <p className="text-xs text-foreground/30">{t('onboarding.ollama.chooseModel')}</p>
                {QUICK_MODELS.map((m) => (
                  <button
                    key={m}
                    onClick={() => void handlePull(m)}
                    disabled={pulling !== null}
                    className="flex w-full items-center justify-between rounded-lg border border-white/[0.07] bg-white/[0.02] px-3 py-2.5 text-left text-sm transition-colors hover:border-white/20 hover:bg-white/[0.04] disabled:opacity-50"
                  >
                    <span className="text-foreground/70">{m}</span>
                    {pulling === m ? (
                      <Loader2 size={13} className="animate-spin text-brand-soft" />
                    ) : (
                      <Download size={13} className="text-foreground/30" />
                    )}
                  </button>
                ))}
              </div>
            )}
          </div>
        ) : (
          <CustomDropdown
            models={models}
            selectedModel={selectedModel}
            onSelectModel={handleSelectModel}
          />
        )}
      </GlassCard>
    </motion.div>
  );
}
