import {
  AlertTriangle,
  ArrowLeft,
  ArrowRight,
  Bot,
  CheckCircle2,
  Cloud,
  Computer,
  Download,
  ExternalLink,
  Eye,
  EyeOff,
  Key,
  Loader2,
  MemoryStick,
  SkipForward,
  WifiOff,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useMemo, useRef, useState } from 'react';

import {
  calculateDownloadSpeed,
  calculateTimeRemaining,
  formatBytes,
  formatDownloadSpeed,
  formatTimeRemaining,
  getRecommended,
  MODEL_RECS,
} from '@ajh/shared';
import { Button, Input, useNotification } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';
import {
  useAIModels,
  useHasProviderKey,
  useJobEvents,
  useOpenExternal,
  usePullModel,
  useSetProviderKey,
  useSystemHealth,
  useSystemResources,
} from '@/services';
import { keys, queryClient } from '@/services/query-client';
import type { AiProvider } from '@/store/preferences-schema';
import { useAIModel, usePreferencesStore } from '@/store/preferences-store';

// Cloud providers available in onboarding (simpler list than settings)
const CLOUD_PROVIDERS: Array<{
  id: AiProvider;
  label: string;
  placeholder: string;
  docsUrl: string;
  color: string;
}> = [
  {
    id: 'openai',
    label: 'OpenAI',
    placeholder: 'sk-...',
    docsUrl: 'https://platform.openai.com/api-keys',
    color: 'text-green-400',
  },
  {
    id: 'anthropic',
    label: 'Anthropic (Claude)',
    placeholder: 'sk-ant-...',
    docsUrl: 'https://console.anthropic.com/settings/keys',
    color: 'text-orange-400',
  },
  {
    id: 'gemini',
    label: 'Google Gemini',
    placeholder: 'AIza...',
    docsUrl: 'https://aistudio.google.com/app/apikey',
    color: 'text-blue-400',
  },
  {
    id: 'openai-compatible',
    label: 'OpenAI-Compatible',
    placeholder: 'API key...',
    docsUrl: 'https://platform.openai.com/docs/api-reference',
    color: 'text-purple-400',
  },
];

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
}

type PullState = 'idle' | 'pulling' | 'done' | 'error';

export function OllamaStep({ onBack, onNext, direction }: Props) {
  const { t } = useTranslation();
  const notify = useNotification();
  const setAIModel = usePreferencesStore((s) => s.setAIModel);
  const setAiProviderConfig = usePreferencesStore((s) => s.setAiProviderConfig);
  const currentAIModel = useAIModel();
  const openExternal = useOpenExternal();
  const pullModel = usePullModel();
  const setProviderKey = useSetProviderKey();

  // Tab: 'local' = Ollama, 'cloud' = cloud provider
  const [mode, setMode] = useState<'local' | 'cloud'>('local');
  const [cloudProvider, setCloudProvider] = useState<AiProvider>('openai');
  const [cloudApiKey, setCloudApiKey] = useState('');
  const [showCloudKey, setShowCloudKey] = useState(false);
  const [savingCloudKey, setSavingCloudKey] = useState(false);

  const cloudMeta = CLOUD_PROVIDERS.find((p) => p.id === cloudProvider) ?? CLOUD_PROVIDERS[0];
  const { data: hasCloudKeyData } = useHasProviderKey(cloudProvider);

  const { data: health, isLoading: healthLoading } = useSystemHealth();
  const { data: modelsRaw, refetch: refetchModels } = useAIModels();
  const models = useMemo(
    () => (modelsRaw as Array<{ name: string }> | undefined) ?? [],
    [modelsRaw]
  );

  const ollamaReady = (health as { ai?: { ready: boolean } } | undefined)?.ai?.ready ?? false;

  const [selectedModel, setSelectedModel] = useState<string>(
    currentAIModel?.defaultModel ?? getRecommended(8).name
  );

  // Use shared hook for system resources and model usage calculations
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
  const [pullState, setPullState] = useState<PullState>('idle');
  const [pullProgress, setPullProgress] = useState(0);
  const [pullJobId, setPullJobId] = useState<string | null>(null);
  const [downloadSpeed, setDownloadSpeed] = useState<string>('');
  const [timeRemaining, setTimeRemaining] = useState<string>('');
  const [downloadedBytes, setDownloadedBytes] = useState<number>(0);
  const [totalBytes, setTotalBytes] = useState<number>(0);
  const [skipping, setSkipping] = useState(false);
  const prevBytesRef = useRef(0);
  const prevTimeRef = useRef(0);
  const lastSpeedUpdateRef = useRef(0);
  const lastTimeUpdateRef = useRef(0);

  // Auto-select a model when models load
  useEffect(() => {
    if (models.length > 0 && !currentAIModel?.defaultModel) {
      const found = models.find((m) => m.name === recommended.name);
      setSelectedModel(found?.name ?? models[0]?.name ?? recommended.name);
    }
  }, [models, currentAIModel?.defaultModel, recommended.name]);

  const installedNames = new Set(models.map((m) => m.name));
  const selectedInstalled = installedNames.has(selectedModel);

  // Listen for job progress events
  useJobEvents((event) => {
    if (event.type === 'job.stream' && event.jobId === pullJobId) {
      const data = event.data as {
        status?: string;
        p?: number;
        completed?: number;
        total?: number;
        digest?: string;
      };
      if (typeof data?.p === 'number') {
        setPullProgress(data.p * 100);
      }

      // Track downloaded and total bytes
      if (typeof data?.completed === 'number') {
        setDownloadedBytes(data.completed);
      }
      // Total bytes should update immediately (no throttle) since it's a one-time value
      if (typeof data?.total === 'number' && data.total > 0) {
        setTotalBytes(data.total);
      }

      // Calculate download speed
      if (typeof data?.completed === 'number' && typeof data?.total === 'number') {
        const now = Date.now();
        const bytes = data.completed;
        const prevBytes = prevBytesRef.current;
        const prevTime = prevTimeRef.current;

        if (prevTime > 0 && bytes > prevBytes) {
          const bytesPerSecond = calculateDownloadSpeed(bytes, prevBytes, now, prevTime);

          if (bytesPerSecond > 0) {
            // Throttle speed updates to every 500ms
            if (now - lastSpeedUpdateRef.current > 500) {
              setDownloadSpeed(formatDownloadSpeed(bytesPerSecond));
              lastSpeedUpdateRef.current = now;
            }

            // Calculate time remaining (throttled to 500ms)
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
        void refetchModels();
        queryClient.invalidateQueries({ queryKey: keys.ai.models });
        notify(`${selectedModel} downloaded successfully.`, 'success');
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
      void refetchModels();
      queryClient.invalidateQueries({ queryKey: keys.ai.models });
      notify(`${selectedModel} downloaded successfully.`, 'success');
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
      notify('Download failed.', 'error');
    }
  });

  const handlePull = async () => {
    setPullState('pulling');
    setPullProgress(0);
    try {
      const result = await pullModel.mutateAsync(selectedModel);
      setPullJobId(result.jobId);
    } catch (err) {
      setPullState('error');
      notify(err instanceof Error ? err.message : 'Download failed.', 'error');
    }
  };

  const hasCloudKey = hasCloudKeyData?.has ?? false;

  const handleSaveCloudKey = async () => {
    if (!cloudApiKey.trim()) return;
    setSavingCloudKey(true);
    try {
      await setProviderKey.mutateAsync({ provider: cloudProvider, apiKey: cloudApiKey.trim() });
      setCloudApiKey('');
      setAiProviderConfig({
        activeProvider: cloudProvider,
        providers: { [cloudProvider]: { model: CLOUD_DEFAULT_MODELS[cloudProvider] ?? '' } },
      });
      notify(`${cloudMeta?.label ?? cloudProvider} API key saved.`, 'success');
    } catch (err) {
      notify(err instanceof Error ? err.message : 'Failed to save key.', 'error');
    } finally {
      setSavingCloudKey(false);
    }
  };

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

  const canContinue =
    mode === 'cloud' ? hasCloudKey : ollamaReady && selectedInstalled && !tooHeavy;

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
        <div className="mb-5 text-center">
          <h1 className="mb-2 text-xl font-semibold text-foreground/95">
            {t('onboarding.ollama.title')}
          </h1>
          <p className="text-sm text-foreground/50">{t('onboarding.ollama.subtitle')}</p>
        </div>

        {/* Local / Cloud tab switcher */}
        <div className="mb-5 flex rounded-xl border border-white/[0.07] bg-white/[0.02] p-1">
          {[
            { id: 'local' as const, label: 'Local (Ollama)', icon: Computer },
            { id: 'cloud' as const, label: 'Cloud AI', icon: Cloud },
          ].map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              onClick={() => setMode(id)}
              className={`flex flex-1 items-center justify-center gap-2 rounded-lg py-2 text-sm font-medium transition-all duration-150 ${
                mode === id
                  ? 'bg-brand/15 text-brand-soft border border-brand/30'
                  : 'text-foreground/40 hover:text-foreground/70'
              }`}
            >
              <Icon size={14} />
              {label}
            </button>
          ))}
        </div>

        {/* Content — switches between not-installed and model selection */}
        <motion.div layout className="overflow-hidden">
          <AnimatePresence mode="wait">
            {/* ── Cloud provider panel ──────────────────────── */}
            {mode === 'cloud' && (
              <motion.div
                key="cloud-panel"
                initial={{ opacity: 0, y: 20, scale: 0.95 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                exit={{ opacity: 0, y: -20, scale: 0.95 }}
                transition={transition.normal}
                className="mb-6 space-y-4"
              >
                {/* Provider selector */}
                <div className="space-y-2">
                  {CLOUD_PROVIDERS.map((p) => (
                    <button
                      key={p.id}
                      onClick={() => setCloudProvider(p.id)}
                      className={`flex w-full items-center gap-3 rounded-xl border px-4 py-2.5 text-left transition-all duration-150 ${
                        cloudProvider === p.id
                          ? 'border-brand/40 bg-brand/10'
                          : 'border-white/[0.07] bg-white/[0.02] hover:border-white/20'
                      }`}
                    >
                      <Bot
                        size={14}
                        className={cloudProvider === p.id ? p.color : 'text-foreground/30'}
                      />
                      <span
                        className={`text-sm font-medium ${cloudProvider === p.id ? 'text-foreground/90' : 'text-foreground/60'}`}
                      >
                        {p.label}
                      </span>
                      {cloudProvider === p.id && hasCloudKey && (
                        <CheckCircle2 size={12} className="ml-auto text-emerald-400" />
                      )}
                    </button>
                  ))}
                </div>

                {/* API key input */}
                {hasCloudKey ? (
                  <div className="flex items-center gap-2 rounded-xl border border-emerald-400/20 bg-emerald-400/5 px-4 py-2.5">
                    <Key size={13} className="text-emerald-400" />
                    <span className="text-sm text-emerald-300/80">
                      {t('settings.aiProvider.keyStored')}
                    </span>
                  </div>
                ) : (
                  <div className="space-y-2">
                    <p className="text-xs text-foreground/35">
                      {t('settings.aiProvider.getKeyAt')}{' '}
                      <button
                        onClick={() => void openExternal.mutateAsync(cloudMeta?.docsUrl ?? '')}
                        className="text-brand-soft/70 underline underline-offset-2 hover:text-brand-soft"
                      >
                        {(cloudMeta?.docsUrl ?? '').replace('https://', '')}
                      </button>
                    </p>
                    <div className="flex flex-col gap-2">
                      <div className="relative">
                        <Input
                          type={showCloudKey ? 'text' : 'password'}
                          value={cloudApiKey}
                          onChange={(e) => setCloudApiKey(e.target.value)}
                          onKeyDown={(e) => e.key === 'Enter' && void handleSaveCloudKey()}
                          placeholder={cloudMeta?.placeholder ?? '…'}
                          className="w-full pr-9 text-sm"
                        />
                        <button
                          onClick={() => setShowCloudKey((v) => !v)}
                          className="absolute right-2.5 top-1/2 -translate-y-1/2 text-foreground/30 hover:text-foreground/60"
                        >
                          {showCloudKey ? <EyeOff size={13} /> : <Eye size={13} />}
                        </button>
                      </div>
                      <div className="flex justify-end">
                        <Button
                          variant="glass"
                          size="sm"
                          disabled={!cloudApiKey.trim() || savingCloudKey}
                          onClick={() => void handleSaveCloudKey()}
                          className={cloudApiKey.trim() && !savingCloudKey ? 'glow-subtle' : ''}
                        >
                          {savingCloudKey ? (
                            <Loader2 size={13} className="animate-spin" />
                          ) : (
                            t('settings.aiProvider.saveKey')
                          )}
                        </Button>
                      </div>
                    </div>
                  </div>
                )}
              </motion.div>
            )}

            {mode === 'local' && healthLoading ? (
              <motion.div
                key="local-checking"
                initial={{ opacity: 0, y: 20, scale: 0.95 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                exit={{ opacity: 0, y: -20, scale: 0.95 }}
                transition={transition.normal}
                className="mb-6 flex flex-col items-center gap-3 py-6"
              >
                <Loader2 size={24} className="animate-spin text-brand-soft" />
                <p className="text-sm text-foreground/40">{t('onboarding.ollama.checking')}</p>
              </motion.div>
            ) : mode === 'local' && !ollamaReady ? (
              /* ── Ollama not found ─────────────────────────── */
              <motion.div
                key="local-not-installed"
                initial={{ opacity: 0, y: 20, scale: 0.95 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                exit={{ opacity: 0, y: -20, scale: 0.95 }}
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
                    queryClient.invalidateQueries({ queryKey: keys.system.health });
                    queryClient.invalidateQueries({ queryKey: keys.ai.models });
                  }}
                >
                  <Loader2 size={12} />
                  {t('onboarding.ollama.recheck')}
                </Button>
              </motion.div>
            ) : mode === 'local' ? (
              /* ── Ollama ready — model selection ──────────── */
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
                  <span className="text-sm text-emerald-200/80">
                    {t('onboarding.ollama.ready')}
                  </span>
                </div>

                {/* Hardware info */}
                <div className="relative group">
                  <div className="flex items-center gap-3 rounded-xl border border-white/[0.06] bg-white/[0.02] px-4 py-3 cursor-help">
                    <MemoryStick size={14} className="text-foreground/30" />
                    <div className="flex-1">
                      <span className="text-xs text-foreground/40">System Performance</span>
                    </div>
                    <span className={`text-xs font-medium ${deviceTier.color}`}>
                      {deviceTier.label}
                    </span>
                  </div>
                  {/* Popover with detailed info */}
                  <div className="absolute left-0 top-full z-50 mt-2 w-64 rounded-xl border border-white/[0.1] bg-black/95 p-4 opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all duration-200 shadow-2xl">
                    <div className="space-y-3">
                      <div className="flex items-center justify-between">
                        <span className="text-xs text-foreground/50">RAM</span>
                        <span className="text-xs font-medium text-foreground/90">
                          {totalRamGb} GB ({freeRamGb} GB free)
                        </span>
                      </div>
                      {cpuCount && (
                        <div className="flex items-center justify-between">
                          <span className="text-xs text-foreground/50">CPU</span>
                          <span className="text-xs font-medium text-foreground/90">
                            {cpuCount} cores
                          </span>
                        </div>
                      )}
                      {hasGpu && (
                        <div className="flex items-center justify-between">
                          <span className="text-xs text-foreground/50">VRAM</span>
                          <span className="text-xs font-medium text-foreground/90">
                            {usedVramGb} / {totalVramGb} GB ({freeVramGb} GB free)
                          </span>
                        </div>
                      )}
                    </div>
                  </div>
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
                    const recTooHeavy = rec.minRamGb > totalRamGb + 2;
                    const recMightLagRam = !recTooHeavy && rec.estimatedRamGb > freeRamGb;
                    const recMightLagVram =
                      hasGpu && rec.estimatedVramGb && rec.estimatedVramGb > freeVramGb;

                    return (
                      <button
                        key={rec.name}
                        onClick={() => !recTooHeavy && setSelectedModel(rec.name)}
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
                                className={`text-sm font-medium ${isSelected ? 'text-foreground/90' : 'text-foreground/70'}`}
                              >
                                {rec.label}
                              </span>
                              {isRec && (
                                <span className="rounded-full border border-brand/30 bg-brand/15 px-1.5 py-0.5 text-[10px] font-medium text-brand-soft">
                                  {t('onboarding.ollama.recommended')}
                                </span>
                              )}
                              {installed && (
                                <span className="rounded-full border border-emerald-400/30 bg-emerald-400/10 px-1.5 py-0.5 text-[10px] font-medium text-emerald-300">
                                  {t('onboarding.ollama.installed')}
                                </span>
                              )}
                              {recMightLagRam && (
                                <span className="rounded-full border border-amber-400/30 bg-amber-400/10 px-1.5 py-0.5 text-[10px] font-medium text-amber-300">
                                  May lag (RAM)
                                </span>
                              )}
                              {recMightLagVram && (
                                <span className="rounded-full border border-orange-400/30 bg-orange-400/10 px-1.5 py-0.5 text-[10px] font-medium text-orange-300">
                                  May lag (VRAM)
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
                            <p className="text-xs text-red-400">
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
                                ? `${t('onboarding.ollama.downloading')} ${selectedModel}…`
                                : `${selectedModel} ${t('onboarding.ollama.downloaded')}`}
                            </span>
                            <div className="flex items-center gap-2">
                              {downloadedBytes > 0 && (
                                <span className="text-foreground/30">
                                  {formatBytes(downloadedBytes)}
                                  {totalBytes > 0 && `/${formatBytes(totalBytes)}`}
                                </span>
                              )}
                              {downloadSpeed && (
                                <span className="text-foreground/30">{downloadSpeed}</span>
                              )}
                              {timeRemaining && (
                                <span className="text-foreground/30">{timeRemaining}</span>
                              )}
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
            ) : null}
          </AnimatePresence>
        </motion.div>

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
