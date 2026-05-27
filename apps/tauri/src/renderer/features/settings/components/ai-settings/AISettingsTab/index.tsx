import { motion } from 'motion/react';
import { useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import { GlassCard, transition, useNotification } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';
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
  useTestProviderKey,
} from '@/services';
import { keys } from '@/services/query-client';
import type { AiProvider } from '@/store/preferences-schema';
import { useAIModel, useAiProviderConfig, usePreferencesStore } from '@/store/preferences-store';
import type { Model } from '@/types';

import { ActiveProviderSwitcher } from './ActiveProviderSwitcher';
import { PROVIDER_ORDER, PROVIDERS } from './provider-meta';
import { ProviderRow } from './ProviderRow';

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
  const testProviderKey = useTestProviderKey();

  // Which provider row is expanded for editing
  const [expanded, setExpanded] = useState<AiProvider | null>(null);
  const [apiKeyInput, setApiKeyInput] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [savingKey, setSavingKey] = useState<AiProvider | null>(null);
  const [testingKey, setTestingKey] = useState<AiProvider | null>(null);
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

  const handleTestKey = async (provider: AiProvider) => {
    const meta = PROVIDERS[provider];
    setTestingKey(provider);
    try {
      const result = await testProviderKey.mutateAsync({ provider });
      if (result.success) {
        notify(`${meta.label} API key is valid!`, 'success');
      } else {
        notify(`API key test failed: ${result.error ?? 'Unknown error'}`, 'error');
      }
    } catch (err) {
      notify(err instanceof Error ? err.message : 'Failed to test key.', 'error');
    } finally {
      setTestingKey(null);
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
      <ActiveProviderSwitcher
        providers={connectedProviders}
        meta={PROVIDERS}
        activeProvider={activeProvider}
        onSetActive={setActiveProvider}
      />

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
              <ProviderRow
                key={p}
                provider={p}
                meta={m}
                connected={connected}
                isActive={isActive}
                isExpanded={isExpanded}
                isSaving={isSaving}
                isTesting={testingKey === p}
                providerModel={providerModel}
                ollamaModels={ollamaModels}
                expandedModels={expandedModels}
                loadingOllama={loadingOllama}
                pulling={pulling}
                apiKeyInput={apiKeyInput}
                showKey={showKey}
                baseUrlInput={baseUrlInput}
                onToggleExpand={() => {
                  setExpanded(isExpanded ? null : p);
                  setApiKeyInput('');
                  setShowKey(false);
                }}
                onTestKey={() => void handleTestKey(p)}
                onRemoveKey={() => void handleRemoveKey(p)}
                onSelectModel={handleSelectModel}
                onPullOllama={handlePullOllama}
                onSetActive={() => setActiveProvider(p)}
                onApiKeyChange={setApiKeyInput}
                onToggleShowKey={() => setShowKey((v: boolean) => !v)}
                onBaseUrlChange={setBaseUrlInput}
                onSaveKey={() => void handleSaveKey(p)}
                onOpenDocs={() => void openExternal.mutateAsync(m.docsUrl)}
                onRecheck={() => {
                  qc.invalidateQueries({ queryKey: keys.system.health });
                  qc.invalidateQueries({ queryKey: keys.ai.models });
                }}
              >
                {p === 'ollama' && connected && (
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
                  </div>
                )}
              </ProviderRow>
            );
          })}
        </div>
      </GlassCard>
    </motion.div>
  );
}
