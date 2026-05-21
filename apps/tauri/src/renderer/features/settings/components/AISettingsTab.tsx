import { RefreshCw } from 'lucide-react';
import { motion } from 'motion/react';
import { useQueryClient } from '@tanstack/react-query';

import { Button, GlassCard } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';
import { useAIModels } from '@/services';
import { keys } from '@/services/query-client';
import { useAIModel, usePreferencesStore } from '@/store/preferences-store';
import type { Model } from '@/types';

import { CustomDropdown } from './CustomDropdown';

export function AISettingsTab() {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const { data: modelList = [], isFetching: loadingModels } = useAIModels();
  const models = modelList as Model[];
  const aiModel = useAIModel();
  const setAIModel = usePreferencesStore((state) => state.setAIModel);

  const selectedModel = aiModel?.defaultModel || '';

  const handleSelectModel = (modelName: string) => {
    setAIModel({
      defaultModel: modelName,
      temperature: aiModel?.temperature || 0.7,
      maxTokens: aiModel?.maxTokens || 2048,
    });
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={transition.normal}
    >
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
          <div className="text-sm text-foreground/50">{t('settings.aiModel.noModels')}</div>
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
