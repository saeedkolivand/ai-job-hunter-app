import { AlertTriangle, ArrowLeft, ArrowRight, Bot, SkipForward } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useMemo, useState } from 'react';

import { getRecommended } from '@ajh/shared';
import { Button, FloatingIcon } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { useAIModels, useSystemHealth, useSystemResources } from '@/services';
import { keys, queryClient } from '@/services/query-client';
import type { AiProvider } from '@/store/preferences-schema';
import { useAIModel, usePreferencesStore } from '@/store/preferences-store';

import { OnboardingStepWrapper } from '../components/OnboardingStepWrapper';
import { CloudProviderPanel } from './ollama/CloudProviderPanel';
import { ModelSelectionPanel } from './ollama/ModelSelectionPanel';
import { OllamaCheckingState } from './ollama/OllamaCheckingState';
import { OllamaNotInstalled } from './ollama/OllamaNotInstalled';
import { TabSwitcher } from './ollama/TabSwitcher';

const CLOUD_DEFAULT_MODELS: Record<string, string> = {
  openai: 'gpt-4o',
  anthropic: 'claude-sonnet-4-6',
  gemini: 'gemini-2.0-flash',
  'openai-compatible': 'gpt-4o',
};

interface Props {
  onBack: () => void;
  onNext: () => void;
  direction: number;
  stepIndex: number;
  totalSteps: number;
}

export function OllamaStep({ onBack, onNext, direction, stepIndex, totalSteps }: Props) {
  const { t } = useTranslation();
  const setAIModel = usePreferencesStore((s) => s.setAIModel);
  const setAiProviderConfig = usePreferencesStore((s) => s.setAiProviderConfig);
  const currentAIModel = useAIModel();

  const [mode, setMode] = useState<'local' | 'cloud'>('local');
  const [cloudProvider, setCloudProvider] = useState<AiProvider>('openai');
  const [skipping, setSkipping] = useState(false);

  const { data: health, isLoading: healthLoading } = useSystemHealth();
  const { data: modelsRaw } = useAIModels();
  const models = useMemo(
    () => (modelsRaw as Array<{ name: string }> | undefined) ?? [],
    [modelsRaw]
  );

  const ollamaReady = (health as { ai?: { ready: boolean } } | undefined)?.ai?.ready ?? false;

  const [selectedModel, setSelectedModel] = useState<string>(
    currentAIModel?.defaultModel ?? getRecommended(8).name
  );

  const { resources, modelUsage } = useSystemResources(selectedModel);
  const {
    totalRamGb,
    freeRamGb,
    deviceTier,
    hasGpu,
    freeVramGb,
    totalVramGb,
    usedVramGb,
    cpuCount,
  } = resources;
  const { tooHeavy } = modelUsage;
  const recommended = getRecommended(totalRamGb);

  // Auto-select a model when models load
  useEffect(() => {
    if (models.length > 0 && !currentAIModel?.defaultModel) {
      const found = models.find((m) => m.name === recommended.name);
      setSelectedModel(found?.name ?? models[0]?.name ?? recommended.name);
    }
  }, [models, currentAIModel?.defaultModel, recommended.name]);

  const installedNames = new Set(models.map((m) => m.name));
  const selectedInstalled = installedNames.has(selectedModel);

  const handleContinue = () => {
    if (mode === 'cloud') {
      setAiProviderConfig({
        activeProvider: cloudProvider,
        providers: { [cloudProvider]: { model: CLOUD_DEFAULT_MODELS[cloudProvider] ?? '' } },
      });
    } else if (selectedModel) {
      setAIModel({ defaultModel: selectedModel, temperature: 0.7, maxTokens: 2048 });
      setAiProviderConfig({
        activeProvider: 'ollama',
        providers: { ollama: { model: selectedModel } },
      });
    }
    onNext();
  };

  const handleSkip = () => {
    setSkipping(true);
    setTimeout(() => onNext(), 1200);
  };

  const handleRecheck = () => {
    queryClient.invalidateQueries({ queryKey: keys.system.health });
    queryClient.invalidateQueries({ queryKey: keys.ai.models });
  };

  const canContinue = mode === 'cloud' ? true : ollamaReady && selectedInstalled && !tooHeavy;

  return (
    <OnboardingStepWrapper
      direction={direction}
      stepIndex={stepIndex}
      totalSteps={totalSteps}
      onBack={onBack}
      onNext={onNext}
      canAdvance={canContinue}
    >
      {/* Icon */}
      <div className="mb-6 flex justify-center">
        <FloatingIcon icon={Bot} size={24} />
      </div>

      {/* Heading */}
      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ delay: 0.1 }}
        className="mb-5 text-center"
      >
        <h1 className="mb-2 text-xl font-semibold text-foreground/95">
          {t('onboarding.ollama.title')}
        </h1>
        <p className="text-sm text-foreground/50">{t('onboarding.ollama.subtitle')}</p>
      </motion.div>

      {/* Tab switcher */}
      <TabSwitcher mode={mode} onModeChange={setMode} />

      {/* Content */}
      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ delay: 0.2 }}
        layout
        className="overflow-hidden"
      >
        <AnimatePresence mode="wait">
          {mode === 'cloud' && (
            <CloudProviderPanel
              key="cloud-panel"
              selectedProvider={cloudProvider}
              onProviderChange={setCloudProvider}
              onContinue={handleContinue}
            />
          )}

          {mode === 'local' && healthLoading && (
            <OllamaCheckingState key="local-checking" message={t('onboarding.ollama.checking')} />
          )}

          {mode === 'local' && !healthLoading && !ollamaReady && (
            <OllamaNotInstalled key="local-not-installed" onRecheck={handleRecheck} />
          )}

          {mode === 'local' && !healthLoading && ollamaReady && (
            <ModelSelectionPanel
              key="local-model-select"
              selectedModel={selectedModel}
              onModelSelect={setSelectedModel}
              installedModels={installedNames}
              totalRamGb={totalRamGb}
              freeRamGb={freeRamGb}
              hasGpu={hasGpu}
              freeVramGb={freeVramGb}
              totalVramGb={totalVramGb}
              usedVramGb={usedVramGb}
              cpuCount={cpuCount}
              deviceTier={deviceTier}
              recommendedModel={recommended}
            />
          )}
        </AnimatePresence>
      </motion.div>

      {/* Actions */}
      <motion.div
        initial={{ y: 10, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ delay: 0.15 }}
        className="flex items-center gap-3"
      >
        <Button variant="ghost" size="sm" onClick={onBack} className="flex items-center gap-1.5">
          <ArrowLeft size={13} />
          {t('onboarding.ollama.back')}
        </Button>

        <div className="flex-1" />

        {!canContinue && !skipping && (
          <Button
            variant="ghost"
            size="sm"
            onClick={handleSkip}
            className="flex items-center gap-1.5 text-foreground/35 hover:text-foreground/60"
          >
            <SkipForward size={12} />
            {t('onboarding.ollama.skip')}
          </Button>
        )}

        {skipping && (
          <motion.div
            initial={{ opacity: 0, scale: 0.9 }}
            animate={{ opacity: 1, scale: 1 }}
            className="flex items-center gap-2 text-xs text-amber-400/70"
          >
            <AlertTriangle size={12} />
            {t('onboarding.ollama.skipWarning')}
          </motion.div>
        )}

        <Button
          variant="default"
          size="sm"
          onClick={handleContinue}
          disabled={!canContinue}
          className="flex items-center gap-1.5"
        >
          {t('onboarding.ollama.next')}
          <ArrowRight size={13} />
        </Button>
      </motion.div>
    </OnboardingStepWrapper>
  );
}
