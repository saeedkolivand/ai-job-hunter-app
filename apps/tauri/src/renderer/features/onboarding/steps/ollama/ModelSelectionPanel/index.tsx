import { CheckCircle2, Download, Loader2 } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';

import { formatBytes, MODEL_RECS } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, transition } from '@ajh/ui';

import { HardwarePopover } from './HardwarePopover';
import { ModelCard } from './ModelCard';
import { useModelPull } from './useModelPull';

interface ModelSelectionPanelProps {
  selectedModel: string;
  onModelSelect: (model: string) => void;
  installedModels: Set<string>;
  totalRamGb: number;
  freeRamGb: number;
  hasGpu: boolean;
  freeVramGb: number;
  totalVramGb: number;
  usedVramGb: number;
  cpuCount?: number;
  deviceTier: { label: string; color: string };
  recommendedModel: { name: string };
  onDownloadComplete?: () => void;
}

export function ModelSelectionPanel({
  selectedModel,
  onModelSelect,
  installedModels,
  totalRamGb,
  freeRamGb,
  hasGpu,
  freeVramGb,
  totalVramGb,
  usedVramGb,
  cpuCount,
  deviceTier,
  recommendedModel,
  onDownloadComplete,
}: ModelSelectionPanelProps) {
  const { t } = useTranslation();
  const {
    pullState,
    pullProgress,
    downloadSpeed,
    timeRemaining,
    downloadedBytes,
    totalBytes,
    handlePull,
  } = useModelPull({ selectedModel, onDownloadComplete });

  const selectedInstalled = installedModels.has(selectedModel);

  return (
    <motion.div
      key="local-model-select"
      initial={{ opacity: 0, y: 20, scale: 0.95 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: -20, scale: 0.95 }}
      transition={transition.normal}
      className="mb-6 space-y-4"
    >
      {/* Ollama ready badge */}
      <div className="flex items-center gap-2 rounded-xl border border-emerald-400/20 bg-emerald-400/5 px-4 py-2.5">
        <CheckCircle2 size={14} className="text-emerald-400" />
        <span className="text-sm text-emerald-200/80">{t('onboarding.ai.readyBadge')}</span>
      </div>

      <HardwarePopover
        totalRamGb={totalRamGb}
        freeRamGb={freeRamGb}
        hasGpu={hasGpu}
        freeVramGb={freeVramGb}
        totalVramGb={totalVramGb}
        usedVramGb={usedVramGb}
        cpuCount={cpuCount}
        deviceTier={deviceTier}
      />

      {/* Model list */}
      <div className="space-y-2">
        <p className="text-xs font-semibold uppercase tracking-widest text-foreground/55">
          {t('onboarding.ai.chooseModel')}
        </p>
        {MODEL_RECS.map((rec) => {
          const recTooHeavy = rec.minRamGb > totalRamGb + 2;
          return (
            <ModelCard
              key={rec.name}
              rec={rec}
              selected={selectedModel === rec.name}
              recommended={rec.name === recommendedModel.name}
              installed={installedModels.has(rec.name)}
              tooHeavy={recTooHeavy}
              mightLagRam={!recTooHeavy && rec.estimatedRamGb > freeRamGb}
              mightLagVram={Boolean(
                hasGpu && rec.estimatedVramGb && rec.estimatedVramGb > freeVramGb
              )}
              onSelect={() => onModelSelect(rec.name)}
            />
          );
        })}
      </div>

      {/* Pull button or progress */}
      <AnimatePresence mode="wait">
        {!selectedInstalled && (
          <>
            {pullState === 'idle' || pullState === 'error' ? (
              <motion.div
                key="pull-button"
                initial={{ opacity: 0, height: 0 }}
                animate={{ opacity: 1, height: 'auto' }}
                exit={{ opacity: 0, height: 0 }}
                transition={transition.normal}
                className="space-y-2 overflow-hidden"
              >
                {pullState === 'error' && (
                  <p className="text-xs text-red-400">{t('onboarding.ai.downloadFailed')}</p>
                )}
                <Button
                  variant="glass"
                  size="sm"
                  className="w-full justify-center gap-2"
                  onClick={() => void handlePull()}
                >
                  <Download size={13} />
                  {t('onboarding.ai.downloadModel', { model: selectedModel })}
                </Button>
              </motion.div>
            ) : pullState === 'pulling' || pullState === 'done' ? (
              <motion.div
                key="pull-progress"
                initial={{ opacity: 0, height: 0 }}
                animate={{ opacity: 1, height: 'auto' }}
                exit={{ opacity: 0, height: 0 }}
                transition={transition.normal}
                className="space-y-2 overflow-hidden"
              >
                <div className="flex items-center justify-between text-xs text-foreground/40">
                  <span className="flex items-center gap-1.5">
                    {pullState === 'pulling' ? (
                      <Loader2 size={11} className="animate-spin" />
                    ) : (
                      <CheckCircle2 size={11} className="text-emerald-400" />
                    )}
                    {pullState === 'pulling'
                      ? t('onboarding.ai.downloadingModel', { model: selectedModel })
                      : t('onboarding.ai.modelDownloaded', { model: selectedModel })}
                  </span>
                  <div className="flex items-center gap-2">
                    {downloadedBytes > 0 && (
                      <span className="text-foreground/30">
                        {formatBytes(downloadedBytes)}
                        {totalBytes > 0 && `/${formatBytes(totalBytes)}`}
                      </span>
                    )}
                    {downloadSpeed && <span className="text-foreground/30">{downloadSpeed}</span>}
                    {timeRemaining && <span className="text-foreground/30">{timeRemaining}</span>}
                    <span>{Math.round(pullProgress)}%</span>
                  </div>
                </div>
                <div className="h-1 overflow-hidden rounded-full bg-white/[0.06]">
                  <motion.div
                    className={`h-full rounded-full bg-gradient-to-r from-brand via-brand-soft to-brand-soft ${
                      pullState === 'done' ? 'bg-emerald-500' : ''
                    }`}
                    animate={{ width: `${pullProgress}%` }}
                    transition={transition.slow}
                  />
                </div>
              </motion.div>
            ) : null}
          </>
        )}
      </AnimatePresence>
    </motion.div>
  );
}
