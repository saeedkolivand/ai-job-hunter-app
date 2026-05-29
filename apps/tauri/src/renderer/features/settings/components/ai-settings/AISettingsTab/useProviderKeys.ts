import { useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import { useNotification } from '@ajh/ui';

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
  useTestProviderKey,
} from '@/services';
import { keys } from '@/services/query-client';
import type { AiProvider } from '@/store/preferences-schema';
import { useAIModel, useAiProviderConfig, usePreferencesStore } from '@/store/preferences-store';
import type { Model } from '@/types';

import { PROVIDER_ORDER, PROVIDERS } from '../provider-meta';

/** Provider key/model management for AISettingsTab: status, editing state, handlers. */
export function useProviderKeys() {
  const qc = useQueryClient();
  const api = useAppClient();
  const notify = useNotification();

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

  const selectedOllamaModel = providerConfig?.providers?.ollama?.model || aiModel?.defaultModel;

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

  // Custom base URL only applies to the OpenAI-compatible provider. Prefer the
  // in-progress edit, fall back to what's saved in config.
  const baseUrlFor = (p: AiProvider): string | undefined =>
    p === 'openai-compatible'
      ? baseUrlInput.trim() ||
        providerConfig?.providers?.['openai-compatible']?.baseUrl ||
        undefined
      : undefined;

  // Models for the expanded cloud provider
  const expandedIsCloud = expanded !== null && expanded !== 'ollama';
  const { data: expandedModelsRaw = [] } = useListProviderModels(
    expanded ?? 'openai',
    expandedIsCloud && (keyStatus[expanded ?? 'openai'] ?? false),
    baseUrlFor(expanded ?? 'openai')
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
        const models = await api.ai.listProviderModels({ provider, baseUrl: baseUrlFor(provider) });
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
      const result = await testProviderKey.mutateAsync({ provider, baseUrl: baseUrlFor(provider) });
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

  const toggleExpand = (p: AiProvider) => {
    setExpanded(expanded === p ? null : p);
    setApiKeyInput('');
    setShowKey(false);
  };

  const recheck = () => {
    qc.invalidateQueries({ queryKey: keys.system.health });
    qc.invalidateQueries({ queryKey: keys.ai.models });
  };

  const openDocs = (url: string) => void openExternal.mutateAsync(url);

  // Connected providers available to be set active
  const connectedProviders = PROVIDER_ORDER.filter((p) => keyStatus[p]);

  return {
    activeProvider,
    setActiveProvider,
    connectedProviders,
    keyStatus,
    providerConfig,
    selectedOllamaModel,
    ollamaModels,
    loadingOllama,
    expanded,
    expandedModels,
    apiKeyInput,
    showKey,
    savingKey,
    testingKey,
    baseUrlInput,
    pulling,
    handleSelectModel,
    handleSaveKey,
    handleTestKey,
    handleRemoveKey,
    handlePullOllama,
    toggleExpand,
    setApiKeyInput,
    toggleShowKey: () => setShowKey((v: boolean) => !v),
    setBaseUrlInput,
    recheck,
    openDocs,
  };
}
