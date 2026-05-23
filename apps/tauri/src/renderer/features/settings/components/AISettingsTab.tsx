import {
  Bot,
  CheckCircle2,
  Download,
  ExternalLink,
  Eye,
  EyeOff,
  Key,
  Loader2,
  RefreshCw,
  Trash2,
  WifiOff,
} from 'lucide-react';
import { motion } from 'motion/react';
import { useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import { Button, GlassCard, Input, useNotification } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';
import { useAppClient } from '@/providers/AppClientProvider';
import {
  useAIModels,
  useHasProviderKey,
  useListProviderModels,
  useOpenExternal,
  usePullModel,
  useRemoveProviderKey,
  useSetProviderKey,
  useSystemHealth,
  useSystemResources,
} from '@/services';
import { keys } from '@/services/query-client';
import type { AiProvider } from '@/store/preferences-schema';
import { useAIModel, useAiProviderConfig, usePreferencesStore } from '@/store/preferences-store';
import type { Model } from '@/types';

import { CustomDropdown } from './CustomDropdown';

// ─── Provider metadata ────────────────────────────────────────────────────────

interface ProviderMeta {
  label: string;
  description: string;
  docsUrl: string;
  color: string;
  models: string[];
}

const PROVIDERS: Record<AiProvider, ProviderMeta> = {
  ollama: {
    label: 'Ollama (Local)',
    description: 'Run models locally — no API key, no cloud, fully private.',
    docsUrl: 'https://ollama.com',
    color: 'text-emerald-400',
    models: [],
  },
  openai: {
    label: 'OpenAI',
    description: 'GPT-4o, GPT-4 Turbo, and more via the OpenAI API.',
    docsUrl: 'https://platform.openai.com/api-keys',
    color: 'text-green-400',
    models: ['gpt-4o', 'gpt-4o-mini', 'gpt-4-turbo', 'gpt-3.5-turbo', 'o1', 'o1-mini'],
  },
  anthropic: {
    label: 'Anthropic (Claude)',
    description: 'Claude Opus, Sonnet, and Haiku via the Anthropic API.',
    docsUrl: 'https://console.anthropic.com/settings/keys',
    color: 'text-orange-400',
    models: ['claude-opus-4-7', 'claude-sonnet-4-6', 'claude-haiku-4-5-20251001'],
  },
  gemini: {
    label: 'Google Gemini',
    description: 'Gemini 2.0 Flash and Gemini 1.5 Pro via the Gemini API.',
    docsUrl: 'https://aistudio.google.com/app/apikey',
    color: 'text-blue-400',
    models: ['gemini-2.0-flash', 'gemini-1.5-pro', 'gemini-1.5-flash'],
  },
  'openai-compatible': {
    label: 'OpenAI-Compatible',
    description: 'Any server that speaks the OpenAI API: Groq, Together, LM Studio, etc.',
    docsUrl: 'https://platform.openai.com/docs/api-reference',
    color: 'text-purple-400',
    models: [],
  },
};

const PROVIDER_ORDER: AiProvider[] = [
  'ollama',
  'openai',
  'anthropic',
  'gemini',
  'openai-compatible',
];

const QUICK_MODELS = ['llama3.2', 'mistral', 'llama3.1:8b', 'llama3.2:1b'];

// ─── Component ────────────────────────────────────────────────────────────────

export function AISettingsTab() {
  const { t } = useTranslation();
  const notify = useNotification();
  const qc = useQueryClient();
  const api = useAppClient();

  const providerConfig = useAiProviderConfig();
  const setActiveProvider = usePreferencesStore((s) => s.setActiveProvider);
  const setProviderSettings = usePreferencesStore((s) => s.setProviderSettings);
  const setAIModel = usePreferencesStore((s) => s.setAIModel);
  const aiModel = useAIModel();

  const activeProvider: AiProvider = providerConfig?.activeProvider ?? 'ollama';

  // Ollama
  const { data: health } = useSystemHealth();
  const ollamaReady = (health as { ai?: { ready: boolean } } | undefined)?.ai?.ready ?? false;
  const { data: ollamaModelsRaw = [], isFetching: loadingOllama } = useAIModels();
  const ollamaModels = ollamaModelsRaw as Model[];
  const pullModel = usePullModel();
  const openExternal = useOpenExternal();
  const [pulling, setPulling] = useState<string | null>(null);

  // System resources for Ollama
  const selectedOllamaModel = providerConfig?.providers?.ollama?.model || aiModel?.defaultModel;
  const { resources, modelUsage } = useSystemResources(selectedOllamaModel);
  const { totalRamGb, freeRamGb, deviceTier, hasGpu, freeVramGb } = resources;
  const { mightLagRam, mightLagVram } = modelUsage;

  // Key status for every cloud provider (hooks must be called unconditionally)
  const openaiKey = useHasProviderKey('openai');
  const anthropicKey = useHasProviderKey('anthropic');
  const geminiKey = useHasProviderKey('gemini');
  const compatKey = useHasProviderKey('openai-compatible');

  const keyStatus: Record<string, boolean> = {
    ollama: ollamaReady,
    openai: openaiKey.data?.has ?? false,
    anthropic: anthropicKey.data?.has ?? false,
    gemini: geminiKey.data?.has ?? false,
    'openai-compatible': compatKey.data?.has ?? false,
  };

  const setProviderKey = useSetProviderKey();
  const removeProviderKey = useRemoveProviderKey();

  // Which provider row is expanded for editing
  const [expanded, setExpanded] = useState<AiProvider | null>(null);
  const [apiKeyInput, setApiKeyInput] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [savingKey, setSavingKey] = useState<AiProvider | null>(null);
  const [baseUrlInput, setBaseUrlInput] = useState(
    providerConfig?.providers?.['openai-compatible']?.baseUrl ?? ''
  );

  // Models for the expanded cloud provider
  const expandedIsCloud = expanded !== null && expanded !== 'ollama';
  const { data: expandedModelsRaw = [] } = useListProviderModels(
    expanded ?? 'openai',
    expandedIsCloud && (keyStatus[expanded ?? 'openai'] ?? false)
  );
  const expandedModels = expandedModelsRaw as Array<{ name: string }>;

  const handleSelectModel = (provider: AiProvider, model: string) => {
    setProviderSettings(provider, { model });
    if (provider === 'ollama') {
      setAIModel({
        defaultModel: model,
        temperature: aiModel?.temperature ?? 0.7,
        maxTokens: aiModel?.maxTokens ?? 2048,
      });
    }
  };

  const handleSaveKey = async (provider: AiProvider) => {
    if (!apiKeyInput.trim()) return;
    const meta = PROVIDERS[provider];
    setSavingKey(provider);
    try {
      await setProviderKey.mutateAsync({ provider, apiKey: apiKeyInput.trim() });
      setApiKeyInput('');
      try {
        const models = await api.ai.listProviderModels({ provider });
        const count = Array.isArray(models) ? models.length : 0;
        notify(
          count > 0
            ? `${meta.label} connected — ${count} model${count === 1 ? '' : 's'} available.`
            : `${meta.label} key saved, but no models returned. Double-check the key.`,
          count > 0 ? 'success' : 'warning'
        );
        qc.invalidateQueries({ queryKey: [...keys.ai.models, 'provider-models', provider] });
      } catch {
        notify(
          `${meta.label} key saved, but couldn't verify it. Check that it's correct.`,
          'warning'
        );
      }
      // Auto-set as active if no other active provider is connected
      if (!keyStatus[activeProvider] && activeProvider !== 'ollama') {
        setActiveProvider(provider);
      }
    } catch (err) {
      notify(err instanceof Error ? err.message : 'Failed to save key.', 'error');
    } finally {
      setSavingKey(null);
    }
  };

  const handleRemoveKey = async (provider: AiProvider) => {
    const meta = PROVIDERS[provider];
    try {
      await removeProviderKey.mutateAsync({ provider });
      notify(`${meta.label} disconnected.`, 'success');
      if (activeProvider === provider) setActiveProvider('ollama');
    } catch (err) {
      notify(err instanceof Error ? err.message : 'Failed to remove key.', 'error');
    }
  };

  const handlePullOllama = async (model: string) => {
    setPulling(model);
    try {
      await pullModel.mutateAsync(model);
      qc.invalidateQueries({ queryKey: keys.ai.models });
      handleSelectModel('ollama', model);
      notify(`${model} downloaded and selected.`, 'success');
    } catch (err) {
      notify(err instanceof Error ? err.message : 'Download failed.', 'error');
    } finally {
      setPulling(null);
    }
  };

  // Connected providers available to be set active
  const connectedProviders = PROVIDER_ORDER.filter((p) => keyStatus[p]);

  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={transition.normal}
      className="space-y-4"
    >
      {/* Active provider switcher */}
      {connectedProviders.length > 1 && (
        <GlassCard>
          <div className="mb-3 text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
            {t('settings.aiProvider.activeProvider')}
          </div>
          <div className="flex flex-wrap gap-1.5">
            {connectedProviders.map((p) => {
              const m = PROVIDERS[p];
              const isActive = p === activeProvider;
              return (
                <button
                  key={p}
                  onClick={() => setActiveProvider(p)}
                  className={`flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-xs font-medium transition-all ${
                    isActive
                      ? 'border-brand/40 bg-brand/10 text-brand-soft'
                      : 'border-white/[0.07] bg-white/[0.02] text-foreground/50 hover:border-white/20 hover:text-foreground/80'
                  }`}
                >
                  <Bot size={11} className={isActive ? m.color : ''} />
                  {m.label}
                  {isActive && <CheckCircle2 size={10} className="text-brand-soft" />}
                </button>
              );
            })}
          </div>
        </GlassCard>
      )}

      {/* Provider list */}
      <GlassCard>
        <div className="mb-3 text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
          {t('settings.aiProvider.title')}
        </div>
        <div className="space-y-2">
          {PROVIDER_ORDER.map((p) => {
            const m = PROVIDERS[p];
            const connected = keyStatus[p] ?? false;
            const isActive = p === activeProvider;
            const isExpanded = expanded === p;
            const providerModel = providerConfig?.providers?.[p]?.model ?? '';
            const isSaving = savingKey === p;

            return (
              <div
                key={p}
                className={`rounded-xl border transition-all ${isExpanded ? 'border-white/15 bg-white/[0.03]' : 'border-white/[0.06] bg-white/[0.01]'}`}
              >
                {/* Row header */}
                <button
                  onClick={() => {
                    setExpanded(isExpanded ? null : p);
                    setApiKeyInput('');
                    setShowKey(false);
                  }}
                  className="flex w-full items-center gap-3 px-4 py-3 text-left"
                >
                  <Bot size={15} className={connected ? m.color : 'text-foreground/25'} />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2 text-sm font-medium text-foreground/80">
                      {m.label}
                      {isActive && connected && (
                        <span className="rounded-full border border-brand/30 bg-brand/10 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-brand-soft">
                          Active
                        </span>
                      )}
                    </div>
                    <div className="mt-0.5 text-[11px] text-foreground/35">{m.description}</div>
                  </div>
                  {/* Status badge */}
                  {p === 'ollama' ? (
                    connected ? (
                      <span className="flex items-center gap-1 text-[10px] text-emerald-400/80">
                        <CheckCircle2 size={10} /> Running
                      </span>
                    ) : (
                      <span className="flex items-center gap-1 text-[10px] text-amber-400/60">
                        <WifiOff size={10} /> Not detected
                      </span>
                    )
                  ) : connected ? (
                    <span className="flex items-center gap-1 text-[10px] text-emerald-400/80">
                      <Key size={10} /> Connected
                    </span>
                  ) : (
                    <span className="text-[10px] text-foreground/30">Not connected</span>
                  )}
                </button>

                {/* Expanded config */}
                {isExpanded && (
                  <div className="border-t border-white/[0.06] px-4 pb-4 pt-3 space-y-3">
                    {p === 'ollama' ? (
                      /* Ollama config */
                      <>
                        {!connected && (
                          <div className="flex gap-2">
                            <Button
                              variant="ghost"
                              size="sm"
                              className="text-foreground/50"
                              onClick={() => void openExternal.mutateAsync('https://ollama.com')}
                            >
                              <ExternalLink size={11} /> Download Ollama
                            </Button>
                            <Button
                              variant="ghost"
                              size="sm"
                              className="text-foreground/40"
                              onClick={() => {
                                qc.invalidateQueries({ queryKey: keys.system.health });
                                qc.invalidateQueries({ queryKey: keys.ai.models });
                              }}
                            >
                              <Loader2 size={11} /> Recheck
                            </Button>
                          </div>
                        )}
                        <div className="flex items-center justify-between">
                          <span className="text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
                            {t('settings.aiModel.title')}
                          </span>
                          <Button
                            onClick={() => void qc.invalidateQueries({ queryKey: keys.ai.models })}
                            disabled={loadingOllama}
                            className="flex items-center gap-1.5 rounded-lg bg-white/5 px-2 py-1 text-xs text-foreground/60 hover:text-foreground h-auto border-transparent"
                          >
                            <RefreshCw size={11} className={loadingOllama ? 'animate-spin' : ''} />
                            {t('settings.aiModel.refresh')}
                          </Button>
                        </div>
                        {ollamaModels.length === 0 ? (
                          <div className="space-y-2">
                            <p className="text-sm text-foreground/50">
                              {t('settings.aiModel.noModels')}
                            </p>
                            {connected &&
                              QUICK_MODELS.map((qm) => (
                                <button
                                  key={qm}
                                  onClick={() => void handlePullOllama(qm)}
                                  disabled={pulling !== null}
                                  className="flex w-full items-center justify-between rounded-lg border border-white/[0.07] bg-white/[0.02] px-3 py-2 text-left text-sm hover:bg-white/[0.04] disabled:opacity-50"
                                >
                                  <span className="text-foreground/70">{qm}</span>
                                  {pulling === qm ? (
                                    <Loader2 size={13} className="animate-spin text-brand-soft" />
                                  ) : (
                                    <Download size={13} className="text-foreground/30" />
                                  )}
                                </button>
                              ))}
                          </div>
                        ) : (
                          <CustomDropdown
                            models={ollamaModels}
                            selectedModel={providerModel || aiModel?.defaultModel || ''}
                            onSelectModel={(m) => handleSelectModel('ollama', m)}
                          />
                        )}
                        {connected && (
                          <div className="space-y-2">
                            {/* System resources display */}
                            <div className="rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2">
                              <div className="flex items-center justify-between text-xs">
                                <span className="text-foreground/40">
                                  RAM: {totalRamGb} GB ({freeRamGb} GB free)
                                </span>
                                <span className={`font-medium ${deviceTier.color}`}>
                                  {deviceTier.label}
                                </span>
                              </div>
                              {hasGpu && (
                                <div className="mt-1 text-xs text-foreground/40">
                                  VRAM: {freeVramGb} GB free
                                </div>
                              )}
                              {mightLagRam && (
                                <div className="mt-1 text-xs text-amber-400/80">
                                  ⚠️ Selected model may lag due to limited RAM
                                </div>
                              )}
                              {mightLagVram && (
                                <div className="mt-1 text-xs text-orange-400/80">
                                  ⚠️ Selected model may lag due to limited VRAM
                                </div>
                              )}
                            </div>
                            <Button
                              variant="glass"
                              size="sm"
                              onClick={() => setActiveProvider('ollama')}
                              disabled={isActive}
                              className={isActive ? 'opacity-40' : 'glow-subtle'}
                            >
                              {isActive ? 'Currently active' : 'Set as active'}
                            </Button>
                          </div>
                        )}
                      </>
                    ) : (
                      /* Cloud provider config */
                      <>
                        {connected ? (
                          <div className="flex items-center justify-between rounded-xl border border-emerald-400/20 bg-emerald-400/5 px-3 py-2">
                            <div className="flex items-center gap-2 text-sm text-emerald-300/80">
                              <Key size={12} /> {t('settings.aiProvider.keyStored')}
                            </div>
                            <Button
                              variant="ghost"
                              size="sm"
                              className="text-xs text-red-400/60 hover:text-red-400"
                              onClick={() => void handleRemoveKey(p)}
                            >
                              <Trash2 size={11} /> {t('settings.aiProvider.removeKey')}
                            </Button>
                          </div>
                        ) : (
                          <div className="space-y-2">
                            <p className="text-xs text-foreground/40">
                              {t('settings.aiProvider.getKeyAt')}{' '}
                              <button
                                onClick={() => void openExternal.mutateAsync(m.docsUrl)}
                                className="text-brand-soft/70 underline underline-offset-2 hover:text-brand-soft"
                              >
                                {m.docsUrl.replace('https://', '')}
                              </button>
                            </p>
                            <div className="flex flex-col gap-2">
                              <div className="relative">
                                <Input
                                  type={showKey ? 'text' : 'password'}
                                  value={apiKeyInput}
                                  onChange={(e) => setApiKeyInput(e.target.value)}
                                  onKeyDown={(e) => e.key === 'Enter' && void handleSaveKey(p)}
                                  placeholder={t('settings.aiProvider.keyPlaceholder')}
                                  className="w-full pr-9 text-sm"
                                />
                                <button
                                  onClick={() => setShowKey((v) => !v)}
                                  className="absolute right-2.5 top-1/2 -translate-y-1/2 text-foreground/30 hover:text-foreground/60"
                                >
                                  {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
                                </button>
                              </div>
                              <div className="flex justify-end">
                                <Button
                                  variant="glass"
                                  size="sm"
                                  disabled={!apiKeyInput.trim() || isSaving}
                                  onClick={() => void handleSaveKey(p)}
                                  className={apiKeyInput.trim() && !isSaving ? 'glow-subtle' : ''}
                                >
                                  {isSaving ? (
                                    <Loader2 size={13} className="animate-spin" />
                                  ) : (
                                    t('settings.aiProvider.saveKey')
                                  )}
                                </Button>
                              </div>
                            </div>
                          </div>
                        )}

                        {/* Base URL for openai-compatible */}
                        {p === 'openai-compatible' && (
                          <div className="space-y-1.5">
                            <label className="text-xs font-medium uppercase tracking-widest text-foreground/30">
                              {t('settings.aiProvider.baseUrl')}
                            </label>
                            <div className="flex gap-2">
                              <Input
                                value={baseUrlInput}
                                onChange={(e) => setBaseUrlInput(e.target.value)}
                                placeholder="https://api.groq.com/openai/v1"
                                className="flex-1 text-sm"
                              />
                              <Button
                                variant="ghost"
                                size="sm"
                                className="shrink-0"
                                onClick={() =>
                                  setProviderSettings('openai-compatible', {
                                    baseUrl: baseUrlInput || undefined,
                                  })
                                }
                              >
                                {t('settings.aiProvider.saveUrl')}
                              </Button>
                            </div>
                          </div>
                        )}

                        {/* Model selector */}
                        {connected && (
                          <div className="space-y-1.5">
                            <div className="text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
                              {t('settings.aiModel.title')}
                            </div>
                            <CustomDropdown
                              models={
                                expandedModels.length > 0
                                  ? expandedModels
                                  : (PROVIDERS[p]?.models ?? []).map((n) => ({ name: n }))
                              }
                              selectedModel={providerModel}
                              onSelectModel={(m) => handleSelectModel(p, m)}
                            />
                          </div>
                        )}

                        {/* Set active button */}
                        {connected && (
                          <Button
                            variant="glass"
                            size="sm"
                            onClick={() => setActiveProvider(p)}
                            disabled={isActive}
                            className={isActive ? 'opacity-40' : 'glow-subtle'}
                          >
                            {isActive ? 'Currently active' : 'Set as active'}
                          </Button>
                        )}
                      </>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </GlassCard>
    </motion.div>
  );
}
