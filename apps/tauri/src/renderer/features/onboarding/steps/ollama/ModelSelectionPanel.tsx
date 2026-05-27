import { CheckCircle2, Download, Loader2, MemoryStick } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useRef, useState } from 'react';

import {
  calculateDownloadSpeed,
  calculateTimeRemaining,
  formatBytes,
  formatDownloadSpeed,
  formatTimeRemaining,
  MODEL_RECS,
} from '@ajh/shared';
import { Button, useNotification } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { transition } from '@ajh/ui';
import { useJobEvents, usePullModel } from '@/services';

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

type PullState = 'idle' | 'pulling' | 'done' | 'error';

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
  const notify = useNotification();
  const pullModel = usePullModel();

  const [pullState, setPullState] = useState<PullState>('idle');
  const [pullProgress, setPullProgress] = useState(0);
  const [pullJobId, setPullJobId] = useState<string | null>(null);
  const [downloadSpeed, setDownloadSpeed] = useState('');
  const [timeRemaining, setTimeRemaining] = useState('');
  const [downloadedBytes, setDownloadedBytes] = useState(0);
  const [totalBytes, setTotalBytes] = useState(0);
  const prevBytesRef = useRef(0);
  const prevTimeRef = useRef(0);
  const lastSpeedUpdateRef = useRef(0);
  const lastTimeUpdateRef = useRef(0);

  const selectedInstalled = installedModels.has(selectedModel);

  const handlePull = async () => {
    setPullState('pulling');
    setPullProgress(0);
    try {
      const result = await pullModel.mutateAsync(selectedModel);
      setPullJobId((result as { jobId: string }).jobId);
    } catch (err) {
      setPullState('error');
      notify(err instanceof Error ? err.message : 'Download failed.', 'error');
    }
  };

  useJobEvents((event) => {
    if (event.type === 'job.stream' && event.jobId === pullJobId) {
      const data = event.data as {
        status?: string;
        p?: number;
        completed?: number;
        total?: number;
      };
      if (typeof data?.p === 'number') {
        setPullProgress(data.p * 100);
      }

      if (typeof data?.completed === 'number') {
        setDownloadedBytes(data.completed);
      }
      if (typeof data?.total === 'number' && data.total > 0) {
        setTotalBytes(data.total);
      }

      if (typeof data?.completed === 'number' && typeof data?.total === 'number') {
        const now = Date.now();
        const bytes = data.completed;
        const prevBytes = prevBytesRef.current;
        const prevTime = prevTimeRef.current;

        if (prevTime > 0 && bytes > prevBytes) {
          const bytesPerSecond = calculateDownloadSpeed(bytes, prevBytes, now, prevTime);

          if (bytesPerSecond > 0) {
            if (now - lastSpeedUpdateRef.current > 500) {
              setDownloadSpeed(formatDownloadSpeed(bytesPerSecond));
              lastSpeedUpdateRef.current = now;
            }

            if (totalBytes > 0 && downloadedBytes > 0 && downloadedBytes < totalBytes) {
              if (now - lastTimeUpdateRef.current > 500) {
                const remainingSeconds = calculateTimeRemaining(
                  totalBytes,
                  downloadedBytes,
                  bytesPerSecond
                );
                setTimeRemaining(formatTimeRemaining(remainingSeconds));
                lastTimeUpdateRef.current = now;
              }
            }
          }
        }

        prevBytesRef.current = bytes;
        prevTimeRef.current = now;
      }

      if (data?.status === 'success') {
        setPullProgress(100);
        setPullState('done');
        setPullJobId(null);
        setDownloadSpeed('');
        setTimeRemaining('');
        setDownloadedBytes(0);
        setTotalBytes(0);
        prevBytesRef.current = 0;
        prevTimeRef.current = 0;
        lastSpeedUpdateRef.current = 0;
        lastTimeUpdateRef.current = 0;
        notify(t('onboarding.ai.downloaded', { model: selectedModel }), 'success');
        onDownloadComplete?.();
      }
    } else if (event.type === 'job.completed' && event.jobId === pullJobId) {
      setPullProgress(100);
      setPullState('done');
      setPullJobId(null);
      setDownloadSpeed('');
      setTimeRemaining('');
      setDownloadedBytes(0);
      setTotalBytes(0);
      prevBytesRef.current = 0;
      prevTimeRef.current = 0;
      lastSpeedUpdateRef.current = 0;
      lastTimeUpdateRef.current = 0;
      notify(t('onboarding.ai.downloaded', { model: selectedModel }), 'success');
      onDownloadComplete?.();
    } else if (event.type === 'job.failed' && event.jobId === pullJobId) {
      setPullState('error');
      setPullJobId(null);
      setDownloadSpeed('');
      setTimeRemaining('');
      setDownloadedBytes(0);
      setTotalBytes(0);
      prevBytesRef.current = 0;
      prevTimeRef.current = 0;
      lastSpeedUpdateRef.current = 0;
      lastTimeUpdateRef.current = 0;
      notify(t('onboarding.ai.downloadFailed'), 'error');
    }
  });

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

      {/* Hardware info */}
      <div className="relative group">
        <div className="flex items-center gap-3 rounded-xl border border-white/[0.06] bg-white/[0.02] px-4 py-3 cursor-help">
          <MemoryStick size={14} className="text-foreground/30" />
          <div className="flex-1">
            <span className="text-xs text-foreground/40">
              {t('onboarding.ai.systemPerformance')}
            </span>
          </div>
          <span className={`text-xs font-medium ${deviceTier.color}`}>{deviceTier.label}</span>
        </div>
        {/* Popover with detailed info */}
        <div className="absolute left-0 top-full z-50 mt-2 w-64 rounded-xl border border-white/[0.1] bg-black/95 p-4 opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all duration-200 shadow-2xl">
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-xs text-foreground/50">{t('onboarding.ai.ramLabel')}</span>
              <span className="text-xs font-medium text-foreground/90">
                {totalRamGb} GB ({freeRamGb} GB {t('onboarding.ai.free')})
              </span>
            </div>
            {cpuCount && (
              <div className="flex items-center justify-between">
                <span className="text-xs text-foreground/50">{t('onboarding.ai.cpuLabel')}</span>
                <span className="text-xs font-medium text-foreground/90">
                  {cpuCount} {t('onboarding.ai.cores')}
                </span>
              </div>
            )}
            {hasGpu && (
              <div className="flex items-center justify-between">
                <span className="text-xs text-foreground/50">{t('onboarding.ai.vramLabel')}</span>
                <span className="text-xs font-medium text-foreground/90">
                  {usedVramGb} / {totalVramGb} GB ({freeVramGb} GB {t('onboarding.ai.free')})
                </span>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Model list */}
      <div className="space-y-2">
        <p className="text-xs font-medium uppercase tracking-widest text-foreground/30">
          {t('onboarding.ai.chooseModel')}
        </p>
        {MODEL_RECS.map((rec) => {
          const installed = installedModels.has(rec.name);
          const isRec = rec.name === recommendedModel.name;
          const isSelected = selectedModel === rec.name;
          const recTooHeavy = rec.minRamGb > totalRamGb + 2;
          const recMightLagRam = !recTooHeavy && rec.estimatedRamGb > freeRamGb;
          const recMightLagVram = hasGpu && rec.estimatedVramGb && rec.estimatedVramGb > freeVramGb;

          return (
            <button
              key={rec.name}
              onClick={() => !recTooHeavy && onModelSelect(rec.name)}
              disabled={recTooHeavy}
              className={`group relative w-full rounded-xl border px-4 py-3 text-left transition-all duration-150 ${
                isSelected
                  ? 'border-brand/40 bg-brand/10'
                  : recTooHeavy
                    ? 'border-white/[0.04] bg-white/[0.01] opacity-40 cursor-not-allowed'
                    : 'border-white/[0.07] bg-white/[0.02] hover:border-white/20 hover:bg-white/[0.04]'
              }`}
            >
              <div className="flex items-center justify-between gap-3">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span
                      className={`text-sm font-medium ${
                        isSelected ? 'text-foreground/90' : 'text-foreground/70'
                      }`}
                    >
                      {rec.label}
                    </span>
                    {isRec && (
                      <span className="rounded-full border border-brand/30 bg-brand/15 px-1.5 py-0.5 text-[10px] font-medium text-brand-soft">
                        {t('onboarding.ai.recommended')}
                      </span>
                    )}
                    {installed && (
                      <span className="rounded-full border border-emerald-400/30 bg-emerald-400/10 px-1.5 py-0.5 text-[10px] font-medium text-emerald-300">
                        {t('onboarding.ai.installed')}
                      </span>
                    )}
                    {recMightLagRam && (
                      <span className="rounded-full border border-amber-400/30 bg-amber-400/10 px-1.5 py-0.5 text-[10px] font-medium text-amber-300">
                        {t('onboarding.ai.mayLagRam')}
                      </span>
                    )}
                    {recMightLagVram && (
                      <span className="rounded-full border border-orange-400/30 bg-orange-400/10 px-1.5 py-0.5 text-[10px] font-medium text-orange-300">
                        {t('onboarding.ai.mayLagVram')}
                      </span>
                    )}
                  </div>
                  <p className="mt-0.5 text-xs text-foreground/35">{rec.description}</p>
                </div>
                <div className="shrink-0 text-right">
                  <span className="text-xs text-foreground/30">{rec.sizeGb} GB</span>
                </div>
              </div>
            </button>
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
                    className={`h-full rounded-full bg-gradient-to-r from-violet-700 via-brand to-brand-soft ${
                      pullState === 'done' ? 'bg-emerald-500' : ''
                    }`}
                    animate={{ width: `${pullProgress}%` }}
                    transition={{ duration: 0.4 }}
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
