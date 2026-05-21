import {
  AlertTriangle,
  ArrowLeft,
  ArrowRight,
  Bot,
  CheckCircle2,
  ChevronRight,
  Download,
  ExternalLink,
  Loader2,
  Cpu,
  MemoryStick,
  SkipForward,
  WifiOff,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import { Button, useToast } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';
import { useAIModels, usePullModel } from '@/services';
import { useSystemHealth, useSystemMetrics, useOpenExternal } from '@/services';
import { keys } from '@/services/query-client';
import { useAIModel, usePreferencesStore } from '@/store/preferences-store';

interface Props {
  onBack: () => void;
  onNext: () => void;
  direction: number;
}

interface ModelRec {
  name: string;
  label: string;
  description: string;
  sizeGb: number;
  minRamGb: number;
}

const MODEL_RECS: ModelRec[] = [
  {
    name: 'llama3.2:1b',
    label: 'Llama 3.2 (1B)',
    description: 'Ultra-lightweight — works on almost any device',
    sizeGb: 1.3,
    minRamGb: 4,
  },
  {
    name: 'llama3.2',
    label: 'Llama 3.2 (3B)',
    description: 'Great balance of speed and quality for everyday tasks',
    sizeGb: 2.0,
    minRamGb: 6,
  },
  {
    name: 'mistral',
    label: 'Mistral 7B',
    description: 'Strong reasoning, ideal for resume analysis',
    sizeGb: 4.1,
    minRamGb: 10,
  },
  {
    name: 'llama3.1:8b',
    label: 'Llama 3.1 (8B)',
    description: 'Best quality for powerful machines',
    sizeGb: 4.7,
    minRamGb: 12,
  },
];

function getRecommended(totalRamGb: number): ModelRec {
  if (totalRamGb >= 12) return MODEL_RECS[3]!;
  if (totalRamGb >= 10) return MODEL_RECS[2]!;
  if (totalRamGb >= 6) return MODEL_RECS[1]!;
  return MODEL_RECS[0]!;
}

function getRamTier(totalRamGb: number): { label: string; color: string } {
  if (totalRamGb >= 16) return { label: 'High-end', color: 'text-emerald-400' };
  if (totalRamGb >= 8) return { label: 'Mid-range', color: 'text-blue-400' };
  return { label: 'Low-end', color: 'text-amber-400' };
}

type PullState = 'idle' | 'pulling' | 'done' | 'error';

export function OllamaStep({ onBack, onNext, direction }: Props) {
  const { t } = useTranslation();
  const toast = useToast();
  const qc = useQueryClient();
  const setAIModel = usePreferencesStore((s) => s.setAIModel);
  const currentAIModel = useAIModel();
  const openExternal = useOpenExternal();
  const pullModel = usePullModel();

  const { data: health, isLoading: healthLoading } = useSystemHealth();
  const { data: metricsRaw } = useSystemMetrics();
  const metrics = metricsRaw as { totalMemoryMb?: number; memoryMb?: number } | undefined;
  const { data: modelsRaw, refetch: refetchModels } = useAIModels();
  const models = (modelsRaw as Array<{ name: string }> | undefined) ?? [];

  const totalRamGb = Math.round((metrics?.totalMemoryMb ?? 8192) / 1024);
  const recommended = getRecommended(totalRamGb);
  const ramTier = getRamTier(totalRamGb);

  const ollamaReady = (health as { ai?: { ready: boolean } } | undefined)?.ai?.ready ?? false;

  const [selectedModel, setSelectedModel] = useState<string>(
    currentAIModel?.defaultModel ?? recommended.name
  );
  const [pullState, setPullState] = useState<PullState>('idle');
  const [pullProgress, setPullProgress] = useState(0);
  const [skipping, setSkipping] = useState(false);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Auto-select a model when models load
  useEffect(() => {
    if (models.length > 0 && !currentAIModel?.defaultModel) {
      const found = models.find((m) => m.name === recommended.name);
      setSelectedModel(found?.name ?? models[0]?.name ?? recommended.name);
    }
  }, [models, currentAIModel?.defaultModel, recommended.name]);

  const installedNames = new Set(models.map((m) => m.name));
  const selectedInstalled = installedNames.has(selectedModel);

  const handlePull = async () => {
    setPullState('pulling');
    setPullProgress(0);
    // Fake incremental progress while the pull runs
    let p = 0;
    pollRef.current = setInterval(() => {
      p = Math.min(p + Math.random() * 3, 92);
      setPullProgress(p);
    }, 800);
    try {
      await pullModel.mutateAsync(selectedModel);
      clearInterval(pollRef.current!);
      setPullProgress(100);
      setPullState('done');
      await refetchModels();
      qc.invalidateQueries({ queryKey: keys.ai.models });
      toast(`${selectedModel} downloaded successfully.`, 'success');
    } catch (err) {
      clearInterval(pollRef.current!);
      setPullState('error');
      toast(err instanceof Error ? err.message : 'Download failed.', 'error');
    }
  };

  const handleContinue = () => {
    if (selectedModel) {
      setAIModel({ defaultModel: selectedModel, temperature: 0.7, maxTokens: 2048 });
    }
    onNext();
  };

  const handleSkip = () => {
    setSkipping(true);
    setTimeout(() => onNext(), 1200);
  };

  const canContinue = ollamaReady && selectedInstalled;

  return (
    <motion.div
      className="relative z-10 w-full max-w-lg mx-4"
      custom={direction}
      variants={{
        initial: (dir: number) => ({ opacity: 0, x: dir * 60 }),
        animate: { opacity: 1, x: 0 },
        exit: (dir: number) => ({ opacity: 0, x: dir * -60 }),
      }}
      initial="initial"
      animate="animate"
      exit="exit"
      transition={transition.modal}
    >
      <div
        className="rounded-2xl border border-white/[0.08] p-8"
        style={{
          background: 'linear-gradient(145deg, rgba(20,14,36,0.97) 0%, rgba(12,10,24,0.97) 100%)',
          boxShadow:
            '0 32px 80px rgba(0,0,0,0.6), 0 0 0 1px rgba(168,85,247,0.08), inset 0 1px 0 rgba(255,255,255,0.04)',
        }}
      >
        {/* Icon */}
        <div className="mb-6 flex justify-center">
          <div
            className="flex h-14 w-14 items-center justify-center rounded-2xl"
            style={{
              background:
                'linear-gradient(135deg, rgba(168,85,247,0.25) 0%, rgba(99,102,241,0.15) 100%)',
              border: '1px solid rgba(168,85,247,0.3)',
              boxShadow: '0 0 32px rgba(168,85,247,0.2)',
            }}
          >
            <Bot size={24} className="text-brand-soft" />
          </div>
        </div>

        {/* Heading */}
        <div className="mb-6 text-center">
          <h1 className="mb-2 text-xl font-semibold text-foreground/95">
            {t('onboarding.ollama.title')}
          </h1>
          <p className="text-sm text-foreground/50">{t('onboarding.ollama.subtitle')}</p>
        </div>

        {/* Content — switches between not-installed and model selection */}
        <AnimatePresence mode="wait">
          {healthLoading ? (
            <motion.div
              key="checking"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="mb-6 flex flex-col items-center gap-3 py-6"
            >
              <Loader2 size={24} className="animate-spin text-brand-soft" />
              <p className="text-sm text-foreground/40">{t('onboarding.ollama.checking')}</p>
            </motion.div>
          ) : !ollamaReady ? (
            /* ── Ollama not found ─────────────────────────── */
            <motion.div
              key="not-installed"
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -8 }}
              transition={transition.normal}
              className="mb-6 space-y-4"
            >
              <div className="flex items-start gap-3 rounded-xl border border-amber-400/20 bg-amber-400/5 p-4">
                <WifiOff size={16} className="mt-0.5 shrink-0 text-amber-400" />
                <div>
                  <p className="text-sm font-medium text-amber-200">
                    {t('onboarding.ollama.notFound')}
                  </p>
                  <p className="mt-1 text-xs text-amber-200/60">
                    {t('onboarding.ollama.notFoundDesc')}
                  </p>
                </div>
              </div>

              <div className="space-y-2">
                <p className="text-xs font-medium uppercase tracking-widest text-foreground/30">
                  {t('onboarding.ollama.installSteps')}
                </p>
                {['download', 'install', 'run'].map((step, i) => (
                  <div key={step} className="flex items-center gap-3 text-sm text-foreground/60">
                    <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full border border-white/10 text-[10px] text-foreground/30">
                      {i + 1}
                    </span>
                    {t(`onboarding.ollama.step.${step}`)}
                  </div>
                ))}
              </div>

              <Button
                variant="glass"
                size="sm"
                className="w-full justify-center gap-2"
                onClick={() => void openExternal.mutateAsync('https://ollama.com')}
              >
                <ExternalLink size={13} />
                {t('onboarding.ollama.downloadButton')}
              </Button>

              <Button
                variant="ghost"
                size="sm"
                className="w-full justify-center gap-1.5 text-foreground/40 hover:text-foreground/70"
                onClick={() => {
                  qc.invalidateQueries({ queryKey: keys.system.health });
                  qc.invalidateQueries({ queryKey: keys.ai.models });
                }}
              >
                <Loader2 size={12} />
                {t('onboarding.ollama.recheck')}
              </Button>
            </motion.div>
          ) : (
            /* ── Ollama ready — model selection ──────────── */
            <motion.div
              key="model-select"
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -8 }}
              transition={transition.normal}
              className="mb-6 space-y-4"
            >
              {/* Ollama ready badge */}
              <div className="flex items-center gap-2 rounded-xl border border-emerald-400/20 bg-emerald-400/5 px-4 py-2.5">
                <CheckCircle2 size={14} className="text-emerald-400" />
                <span className="text-sm text-emerald-200/80">{t('onboarding.ollama.ready')}</span>
              </div>

              {/* Hardware info */}
              <div className="flex items-center gap-3 rounded-xl border border-white/[0.06] bg-white/[0.02] px-4 py-3">
                <MemoryStick size={14} className="text-foreground/30" />
                <div className="flex-1">
                  <span className="text-xs text-foreground/40">{t('onboarding.ollama.ram')}</span>
                  <span className="ml-2 text-xs font-medium text-foreground/70">
                    {totalRamGb} GB
                  </span>
                </div>
                <span className={`text-xs font-medium ${ramTier.color}`}>{ramTier.label}</span>
              </div>

              {/* Model list */}
              <div className="space-y-2">
                <p className="text-xs font-medium uppercase tracking-widest text-foreground/30">
                  {t('onboarding.ollama.chooseModel')}
                </p>
                {MODEL_RECS.map((rec) => {
                  const installed = installedNames.has(rec.name);
                  const isRec = rec.name === recommended.name;
                  const isSelected = selectedModel === rec.name;
                  const tooHeavy = rec.minRamGb > totalRamGb + 2;

                  return (
                    <button
                      key={rec.name}
                      onClick={() => !tooHeavy && setSelectedModel(rec.name)}
                      disabled={tooHeavy}
                      className={`group relative w-full rounded-xl border px-4 py-3 text-left transition-all duration-150 ${
                        isSelected
                          ? 'border-brand/40 bg-brand/10'
                          : tooHeavy
                            ? 'border-white/[0.04] bg-white/[0.01] opacity-40 cursor-not-allowed'
                            : 'border-white/[0.07] bg-white/[0.02] hover:border-white/20 hover:bg-white/[0.04]'
                      }`}
                    >
                      <div className="flex items-center justify-between gap-3">
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-2">
                            <span
                              className={`text-sm font-medium ${isSelected ? 'text-foreground/90' : 'text-foreground/70'}`}
                            >
                              {rec.label}
                            </span>
                            {isRec && (
                              <span className="rounded-full border border-brand/30 bg-brand/15 px-1.5 py-0.5 text-[10px] font-medium text-brand-soft">
                                {t('onboarding.ollama.recommended')}
                              </span>
                            )}
                            {installed && <CheckCircle2 size={12} className="text-emerald-400" />}
                          </div>
                          <p className="mt-0.5 text-xs text-foreground/35">{rec.description}</p>
                        </div>
                        <div className="shrink-0 text-right">
                          <span className="text-xs text-foreground/30">{rec.sizeGb} GB</span>
                        </div>
                      </div>

                      {/* Selected indicator */}
                      {isSelected && (
                        <div className="absolute right-3 top-1/2 -translate-y-1/2">
                          <ChevronRight size={14} className="text-brand-soft" />
                        </div>
                      )}
                    </button>
                  );
                })}
              </div>

              {/* Pull button or progress */}
              {!selectedInstalled && pullState !== 'done' && (
                <AnimatePresence mode="wait">
                  {pullState === 'idle' || pullState === 'error' ? (
                    <motion.div
                      key="pull-btn"
                      initial={{ opacity: 0 }}
                      animate={{ opacity: 1 }}
                      exit={{ opacity: 0 }}
                    >
                      {pullState === 'error' && (
                        <p className="mb-2 text-xs text-red-400">
                          {t('onboarding.ollama.pullError')}
                        </p>
                      )}
                      <Button
                        variant="glass"
                        size="sm"
                        className="w-full justify-center gap-2"
                        onClick={() => void handlePull()}
                      >
                        <Download size={13} />
                        {t('onboarding.ollama.download')} {selectedModel}
                      </Button>
                    </motion.div>
                  ) : (
                    <motion.div
                      key="pull-progress"
                      initial={{ opacity: 0 }}
                      animate={{ opacity: 1 }}
                      exit={{ opacity: 0 }}
                      className="space-y-2"
                    >
                      <div className="flex items-center justify-between text-xs text-foreground/40">
                        <span className="flex items-center gap-1.5">
                          <Loader2 size={11} className="animate-spin" />
                          {t('onboarding.ollama.downloading')} {selectedModel}…
                        </span>
                        <span>{Math.round(pullProgress)}%</span>
                      </div>
                      <div className="h-1 overflow-hidden rounded-full bg-white/[0.06]">
                        <motion.div
                          className="h-full rounded-full bg-gradient-to-r from-violet-700 via-brand to-brand-soft"
                          animate={{ width: `${pullProgress}%` }}
                          transition={{ duration: 0.4 }}
                        />
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>
              )}
            </motion.div>
          )}
        </AnimatePresence>

        {/* Step dots */}
        <div className="mb-6 flex justify-center gap-1.5">
          {[0, 1, 2, 3].map((i) => (
            <div
              key={i}
              className={`h-1 rounded-full transition-all duration-300 ${
                i === 2 ? 'w-5 bg-brand' : 'w-1.5 bg-white/15'
              }`}
            />
          ))}
        </div>

        {/* Actions */}
        <div className="flex items-center gap-3">
          <Button
            variant="ghost"
            size="sm"
            onClick={onBack}
            className="flex items-center gap-1.5"
            disabled={pullState === 'pulling'}
          >
            <ArrowLeft size={13} />
            {t('onboarding.ollama.back')}
          </Button>

          <div className="flex-1" />

          {/* Skip */}
          {!canContinue && !skipping && (
            <Button
              variant="ghost"
              size="sm"
              onClick={handleSkip}
              className="flex items-center gap-1.5 text-foreground/35 hover:text-foreground/60"
              disabled={pullState === 'pulling'}
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

          {/* Continue */}
          <Button
            variant="default"
            size="sm"
            onClick={handleContinue}
            disabled={!canContinue || pullState === 'pulling'}
            className="flex items-center gap-1.5"
          >
            {t('onboarding.ollama.next')}
            <ArrowRight size={13} />
          </Button>
        </div>
      </div>
    </motion.div>
  );
}
