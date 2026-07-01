import { useState } from 'react';
import { useQueries, useQueryClient } from '@tanstack/react-query';

import { useNotification } from '@ajh/ui';

import { PROVIDER_ORDER, PROVIDERS } from '@/lib/ai-providers/provider-meta';
import { useAppClient } from '@/providers/AppClientProvider';
import {
  useAIModels,
  useListProviderModels,
  useListProviderModelsLazy,
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

  // Ollama (local server) + CLI-agent detection both come from the health probe.
  const { data: health } = useSystemHealth();
  const ollamaReady = health?.ai.ready ?? false;
  const cliAgentStatus = health?.cliAgents ?? {};
  const { data: ollamaModelsRaw = [], isFetching: loadingOllama } = useAIModels();
  const ollamaModels = ollamaModelsRaw as Model[];
  const pullModel = usePullModel();
  const openExternal = useOpenExternal();
  const [pulling, setPulling] = useState<string | null>(null);

  const selectedOllamaModel = providerConfig?.providers?.ollama?.model || aiModel?.defaultModel;

  // Cloud providers authenticate with a stored key — query each one's presence.
  // Driven off the registry so adding a cloud provider needs no new hook line.
  const cloudProviders = PROVIDER_ORDER.filter((p) => PROVIDERS[p].kind === 'cloud');
  const cloudKeyQueries = useQueries({
    queries: cloudProviders.map((p) => ({
      queryKey: [...keys.ai.models, 'provider-key', p],
      queryFn: () => api.ai.hasProviderKey({ provider: p }),
      staleTime: 30_000,
    })),
  });

  // Connection status by provider kind: cloud → stored key; local server → Ollama
  // health; CLI agent → binary detected.
  const keyStatus: Record<string, boolean> = Object.fromEntries(
    PROVIDER_ORDER.map((p): [string, boolean] => {
      const kind = PROVIDERS[p].kind;
      if (kind === 'cloud') {
        return [p, cloudKeyQueries[cloudProviders.indexOf(p)]?.data?.has ?? false];
      }
      if (kind === 'cli-agent') return [p, cliAgentStatus[p]?.detected ?? false];
      return [p, ollamaReady]; // local-server (Ollama)
    })
  );

  const setProviderKey = useSetProviderKey();
  const removeProviderKey = useRemoveProviderKey();
  const testProviderKey = useTestProviderKey();
  const listProviderModelsLazy = useListProviderModelsLazy();

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

  // Models for the expanded non-local provider (cloud key-based, or CLI agents
  // which "list" their aliases through the same IPC path). Ollama uses its own
  // local model list, so it's excluded.
  const expandedFetchesModels = expanded !== null && PROVIDERS[expanded].kind !== 'local-server';
  const { data: expandedModelsRaw = [] } = useListProviderModels(
    expanded ?? 'openai',
    expandedFetchesModels && (keyStatus[expanded ?? 'openai'] ?? false),
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
        // One-shot verification through the service layer (primes the cache too).
        const models = await listProviderModelsLazy.mutateAsync({
          provider,
          baseUrl: baseUrlFor(provider),
        });
        const count = Array.isArray(models) ? models.length : 0;
        notify.open({
          message:
            count > 0
              ? `${meta.label} connected — ${count} model${count === 1 ? '' : 's'} available.`
              : `${meta.label} key saved, but no models returned. Double-check the key.`,
          variant: count > 0 ? 'success' : 'warning',
        });
      } catch {
        notify.warning({
          message: `${meta.label} key saved, but couldn't verify it. Check that it's correct.`,
        });
      }
      // Auto-set as active if no other active provider is connected
      if (!keyStatus[activeProvider] && activeProvider !== 'ollama') {
        setActiveProvider(provider);
      }
    } catch (err) {
      notify.error({ message: err instanceof Error ? err.message : 'Failed to save key.' });
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
        notify.success({ message: `${meta.label} API key is valid!` });
      } else {
        notify.error({ message: `API key test failed: ${result.error ?? 'Unknown error'}` });
      }
    } catch (err) {
      notify.error({ message: err instanceof Error ? err.message : 'Failed to test key.' });
    } finally {
      setTestingKey(null);
    }
  };

  const handleRemoveKey = async (provider: AiProvider) => {
    const meta = PROVIDERS[provider];
    try {
      await removeProviderKey.mutateAsync({ provider });
      notify.success({ message: `${meta.label} disconnected.` });
      if (activeProvider === provider) setActiveProvider('ollama');
    } catch (err) {
      notify.error({ message: err instanceof Error ? err.message : 'Failed to remove key.' });
    }
  };

  const handlePullOllama = async (model: string) => {
    setPulling(model);
    try {
      await pullModel.mutateAsync(model);
      void qc.invalidateQueries({ queryKey: keys.ai.models });
      handleSelectModel('ollama', model);
      notify.success({ message: `${model} downloaded and selected.` });
    } catch (err) {
      notify.error({ message: err instanceof Error ? err.message : 'Download failed.' });
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
    void qc.invalidateQueries({ queryKey: keys.system.health });
    void qc.invalidateQueries({ queryKey: keys.ai.models });
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
