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

import { Button, GlassCard, Input, useToast } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
import { transition } from '@/lib/motion';
import {
  useAIModels,
  useHasProviderKey,
  useListProviderModels,
  useOpenExternal,
  usePullModel,
  useRemoveProviderKey,
  useSetProviderKey,
  useSystemHealth,
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
  const toast = useToast();
  const qc = useQueryClient();

  const providerConfig = useAiProviderConfig();
  const setAiProviderConfig = usePreferencesStore((s) => s.setAiProviderConfig);
  const setAIModel = usePreferencesStore((s) => s.setAIModel);
  const aiModel = useAIModel();

  const activeProvider: AiProvider = providerConfig?.provider ?? 'ollama';
  const meta = PROVIDERS[activeProvider];

  // Ollama
  const { data: health } = useSystemHealth();
  const ollamaReady = (health as { ai?: { ready: boolean } } | undefined)?.ai?.ready ?? false;
  const { data: ollamaModelsRaw = [], isFetching: loadingOllama } = useAIModels();
  const ollamaModels = ollamaModelsRaw as Model[];
  const pullModel = usePullModel();
  const openExternal = useOpenExternal();
  const [pulling, setPulling] = useState<string | null>(null);

  // Cloud provider
  const { data: hasKeyData } = useHasProviderKey(activeProvider);
  const hasKey = hasKeyData?.has ?? false;
  const setProviderKey = useSetProviderKey();
  const removeProviderKey = useRemoveProviderKey();
  const { data: cloudModelsRaw = [] } = useListProviderModels(activeProvider, hasKey);
  const cloudModels = cloudModelsRaw as Array<{ name: string }>;

  const [apiKeyInput, setApiKeyInput] = useState('');
  const [baseUrlInput, setBaseUrlInput] = useState(providerConfig?.baseUrl ?? '');
  const [showKey, setShowKey] = useState(false);
  const [savingKey, setSavingKey] = useState(false);

  const selectedModel = providerConfig?.model || aiModel?.defaultModel || '';

  const handleSelectProvider = (provider: AiProvider) => {
    setAiProviderConfig({ provider, model: '', baseUrl: undefined });
    setApiKeyInput('');
    setShowKey(false);
  };

  const handleSelectModel = (model: string) => {
    setAiProviderConfig({ ...providerConfig, provider: activeProvider, model });
    if (activeProvider === 'ollama') {
      setAIModel({
        defaultModel: model,
        temperature: aiModel?.temperature ?? 0.7,
        maxTokens: aiModel?.maxTokens ?? 2048,
      });
    }
  };

  const handleSaveKey = async () => {
    if (!apiKeyInput.trim()) return;
    setSavingKey(true);
    try {
      await setProviderKey.mutateAsync({ provider: activeProvider, apiKey: apiKeyInput.trim() });
      setApiKeyInput('');
      toast(`${meta.label} API key saved.`, 'success');
    } catch (err) {
      toast(err instanceof Error ? err.message : 'Failed to save key.', 'error');
    } finally {
      setSavingKey(false);
    }
  };

  const handleRemoveKey = async () => {
    try {
      await removeProviderKey.mutateAsync({ provider: activeProvider });
      toast(`${meta.label} API key removed.`, 'success');
    } catch (err) {
      toast(err instanceof Error ? err.message : 'Failed to remove key.', 'error');
    }
  };

  const handlePullOllama = async (model: string) => {
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

  const modelOptions =
    activeProvider === 'ollama'
      ? ollamaModels
      : cloudModels.length > 0
        ? cloudModels
        : (PROVIDERS[activeProvider]?.models ?? []).map((m) => ({ name: m }));

  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={transition.normal}
      className="space-y-4"
    >
      {/* Provider selector */}
      <GlassCard>
        <div className="mb-3 text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
          {t('settings.aiProvider.title')}
        </div>
        <p className="mb-4 text-sm text-foreground/55">{t('settings.aiProvider.description')}</p>
        <div className="space-y-2">
          {PROVIDER_ORDER.map((p) => {
            const m = PROVIDERS[p];
            const isActive = p === activeProvider;
            return (
              <button
                key={p}
                onClick={() => handleSelectProvider(p)}
                className={`flex w-full items-center gap-3 rounded-xl border px-4 py-3 text-left transition-all duration-150 ${
                  isActive
                    ? 'border-brand/40 bg-brand/10'
                    : 'border-white/[0.07] bg-white/[0.02] hover:border-white/20 hover:bg-white/[0.04]'
                }`}
              >
                <Bot size={16} className={isActive ? m.color : 'text-foreground/30'} />
                <div className="min-w-0 flex-1">
                  <div
                    className={`text-sm font-medium ${isActive ? 'text-foreground/90' : 'text-foreground/60'}`}
                  >
                    {m.label}
                  </div>
                  <div className="mt-0.5 text-xs text-foreground/30">{m.description}</div>
                </div>
                {isActive && <CheckCircle2 size={14} className="shrink-0 text-brand-soft" />}
              </button>
            );
          })}
        </div>
      </GlassCard>

      {/* Ollama config */}
      {activeProvider === 'ollama' && (
        <GlassCard>
          <div className="mb-3 flex items-center justify-between">
            <div className="text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
              Ollama
            </div>
            {ollamaReady ? (
              <div className="flex items-center gap-1.5 text-xs text-emerald-400">
                <CheckCircle2 size={12} /> Running
              </div>
            ) : (
              <div className="flex items-center gap-1.5 text-xs text-amber-400/80">
                <WifiOff size={12} /> Not detected
              </div>
            )}
          </div>

          {!ollamaReady && (
            <div className="mb-3 flex gap-2">
              <Button
                variant="ghost"
                size="sm"
                className="flex items-center gap-1.5 text-foreground/50"
                onClick={() => void openExternal.mutateAsync('https://ollama.com')}
              >
                <ExternalLink size={11} /> Download Ollama
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
                <Loader2 size={11} /> Recheck
              </Button>
            </div>
          )}

          <div className="mb-3 flex items-center justify-between">
            <div className="text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
              {t('settings.aiModel.title')}
            </div>
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
            <div className="space-y-3">
              <p className="text-sm text-foreground/50">{t('settings.aiModel.noModels')}</p>
              {ollamaReady && (
                <div className="space-y-2">
                  <p className="text-xs text-foreground/30">{t('onboarding.ollama.chooseModel')}</p>
                  {QUICK_MODELS.map((m) => (
                    <button
                      key={m}
                      onClick={() => void handlePullOllama(m)}
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
              models={ollamaModels}
              selectedModel={selectedModel}
              onSelectModel={handleSelectModel}
            />
          )}
        </GlassCard>
      )}

      {/* Cloud provider config */}
      {activeProvider !== 'ollama' && (
        <GlassCard>
          <div className="mb-3 text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
            {t('settings.aiProvider.apiKey')}
          </div>

          {hasKey ? (
            <div className="mb-4 flex items-center justify-between rounded-xl border border-emerald-400/20 bg-emerald-400/5 px-4 py-2.5">
              <div className="flex items-center gap-2 text-sm text-emerald-300/80">
                <Key size={13} />
                {t('settings.aiProvider.keyStored')}
              </div>
              <Button
                variant="ghost"
                size="sm"
                className="flex items-center gap-1 text-xs text-red-400/60 hover:text-red-400"
                onClick={() => void handleRemoveKey()}
              >
                <Trash2 size={11} /> {t('settings.aiProvider.removeKey')}
              </Button>
            </div>
          ) : (
            <div className="mb-4 space-y-2">
              <p className="text-xs text-foreground/40">
                {t('settings.aiProvider.getKeyAt')}{' '}
                <button
                  onClick={() => void openExternal.mutateAsync(meta.docsUrl)}
                  className="text-brand-soft/70 underline underline-offset-2 hover:text-brand-soft"
                >
                  {meta.docsUrl.replace('https://', '')}
                </button>
              </p>
              <div className="flex gap-2">
                <div className="relative flex-1">
                  <Input
                    type={showKey ? 'text' : 'password'}
                    value={apiKeyInput}
                    onChange={(e) => setApiKeyInput(e.target.value)}
                    onKeyDown={(e) => e.key === 'Enter' && void handleSaveKey()}
                    placeholder={t('settings.aiProvider.keyPlaceholder')}
                    className="pr-9 font-mono text-sm"
                  />
                  <button
                    onClick={() => setShowKey((v) => !v)}
                    className="absolute right-2.5 top-1/2 -translate-y-1/2 text-foreground/30 hover:text-foreground/60"
                  >
                    {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
                  </button>
                </div>
                <Button
                  variant="glass"
                  size="sm"
                  disabled={!apiKeyInput.trim() || savingKey}
                  onClick={() => void handleSaveKey()}
                  className="shrink-0"
                >
                  {savingKey ? (
                    <Loader2 size={13} className="animate-spin" />
                  ) : (
                    t('settings.aiProvider.saveKey')
                  )}
                </Button>
              </div>
            </div>
          )}

          {/* Base URL for openai-compatible */}
          {activeProvider === 'openai-compatible' && (
            <div className="mb-4 space-y-2">
              <label className="text-xs font-medium uppercase tracking-widest text-foreground/30">
                {t('settings.aiProvider.baseUrl')}
              </label>
              <div className="flex gap-2">
                <Input
                  value={baseUrlInput}
                  onChange={(e) => setBaseUrlInput(e.target.value)}
                  placeholder="https://api.groq.com/openai/v1"
                  className="flex-1 font-mono text-sm"
                />
                <Button
                  variant="ghost"
                  size="sm"
                  className="shrink-0"
                  onClick={() =>
                    setAiProviderConfig({
                      provider: activeProvider,
                      model: providerConfig?.model ?? '',
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
          {hasKey && modelOptions.length > 0 && (
            <div className="space-y-2">
              <div className="text-xs font-medium uppercase tracking-[0.16em] text-foreground/40">
                {t('settings.aiModel.title')}
              </div>
              <CustomDropdown
                models={modelOptions}
                selectedModel={selectedModel}
                onSelectModel={handleSelectModel}
              />
            </div>
          )}
        </GlassCard>
      )}
    </motion.div>
  );
}
