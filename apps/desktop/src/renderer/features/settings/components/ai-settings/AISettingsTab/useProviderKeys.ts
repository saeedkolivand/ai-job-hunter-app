import { useState } from 'react';
import { useQueries, useQueryClient } from '@tanstack/react-query';

import { useNotification } from '@ajh/ui';

import { isProviderConfigured, PROVIDER_ORDER, PROVIDERS } from '@/lib/ai-providers/provider-meta';
import { useAppClient } from '@/providers/AppClientProvider';
import {
  useActiveConfig,
  useAIModels,
  useListProviderModels,
  useListProviderModelsLazy,
  useOpenExternal,
  usePullModel,
  useRemoveProviderKey,
  useSetActiveProvider,
  useSetProviderKey,
  useSetProviderSettings,
  useSystemHealth,
  useTestProviderKey,
} from '@/services';
import { keys } from '@/services/query-client';
import type { AiProvider } from '@/store/preferences-schema';
import type { Model } from '@/types';

/** Provider key/model management for AISettingsTab: status, editing state, handlers. */
export function useProviderKeys() {
  const qc = useQueryClient();
  const api = useAppClient();
  const notify = useNotification();

  // Routing is backend-owned (task #16): reads via the active-config query, writes
  // via the switch/edit setters.
  const { data: providerConfig } = useActiveConfig();
  const setActiveProviderMut = useSetActiveProvider();
  const setProviderSettingsMut = useSetProviderSettings();
  const setActiveProvider = (provider: AiProvider) => setActiveProviderMut.mutate(provider);
  const setProviderSettings = (provider: AiProvider, model: string) =>
    setProviderSettingsMut.mutate({ provider, model });

  const activeProvider: AiProvider = (providerConfig?.activeProvider ?? 'ollama') as AiProvider;

  // Ollama (local server) + CLI-agent detection both come from the health probe.
  const { data: health } = useSystemHealth();
  const ollamaReady = health?.ai.ready ?? false;
  const cliAgentStatus = health?.cliAgents ?? {};
  const { data: ollamaModelsRaw = [], isFetching: loadingOllama } = useAIModels();
  const ollamaModels = ollamaModelsRaw as Model[];
  const pullModel = usePullModel();
  const openExternal = useOpenExternal();
  const [pulling, setPulling] = useState<string | null>(null);

  const selectedOllamaModel = providerConfig?.providers?.ollama?.model;

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
  // in-progress edit, fall back to what's saved in config. Resolved ONCE here
  // and reused by every consumer below (including the one handed to
  // `CloudProviderConfig` as `configuredBaseUrl`) — passing the raw
  // `baseUrlInput` to one call site and this resolved value to another is
  // exactly the divergence a single predicate was meant to prevent (they can
  // disagree on whether the provider is "configured" while both claim to
  // share the same check). Also immune to `baseUrlInput`'s `useState`
  // initializer being stale on a cold boot (seeded once from `providerConfig`
  // before it may have loaded): this function re-reads `providerConfig` live
  // on every call, so it self-corrects once the query resolves even if the
  // input's seed never does.
  const baseUrlFor = (p: AiProvider): string | undefined =>
    p === 'openai-compatible'
      ? baseUrlInput.trim() ||
        providerConfig?.providers?.['openai-compatible']?.baseUrl ||
        undefined
      : undefined;
  const configuredBaseUrl = baseUrlFor('openai-compatible');

  // Models for the expanded non-local provider (cloud key-based, or CLI agents
  // which "list" their aliases through the same IPC path). Ollama uses its own
  // local model list, so it's excluded. `openai-compatible` is the one cloud
  // provider the backend lists models for without a bearer header (LM Studio /
  // vLLM are keyless) — gating it on `keyStatus` like every other cloud
  // provider left keyless setups unable to discover a model in Settings at
  // all, even though `ModelSelector` already unblocked the same case. But it
  // must still be configured (a stored/in-progress base URL, or a key) —
  // otherwise it silently falls back to `api.openai.com` server-side.
  const expandedFetchesModels = expanded !== null && PROVIDERS[expanded].kind !== 'local-server';
  // `expandedFetchesModels` already proves `expanded !== null` — one fallback,
  // reused below, instead of five copies that could drift from each other.
  const target = expanded ?? 'openai';
  const expandedCanFetchModels =
    expandedFetchesModels &&
    isProviderConfigured(target, keyStatus[target] ?? false, baseUrlFor(target));
  const {
    data: expandedModelsResult,
    isLoading: expandedModelsLoading,
    isError: expandedModelsErrored,
    error: expandedModelsErrorObj,
  } = useListProviderModels(target, expandedCanFetchModels, baseUrlFor(target));
  const expandedModels = expandedModelsResult?.models ?? [];
  const expandedModelsCached = expandedModelsResult?.cached ?? false;
  // A rejected invoke can reject with a plain string rather than an `Error`
  // (Tauri serializes `AppError` as its message string) — guard both shapes.
  const expandedModelsError = expandedModelsErrored
    ? expandedModelsErrorObj instanceof Error
      ? expandedModelsErrorObj.message
      : String(expandedModelsErrorObj)
    : undefined;

  const handleSelectModel = (provider: AiProvider, model: string) => {
    // Edits the provider's model without flipping the active provider (the
    // switch-vs-edit split); the dead `aiModel` Ollama mirror is gone (task #16).
    setProviderSettings(provider, model);
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
    expandedModelsLoading,
    expandedModelsCached,
    expandedModelsError,
    apiKeyInput,
    showKey,
    savingKey,
    testingKey,
    baseUrlInput,
    configuredBaseUrl,
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
