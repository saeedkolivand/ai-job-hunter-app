import { Cpu } from 'lucide-react';
import { useQueries } from '@tanstack/react-query';

import { Dropdown } from '@ajh/ui';

import { PROVIDER_ORDER, PROVIDERS } from '@/lib/ai-providers/provider-meta';
import { useAppClient } from '@/providers/AppClientProvider';
import { useAIModels, useHasProviderKey, useSystemHealth } from '@/services';
import { keys } from '@/services/query-client';
import type { AiProvider } from '@/store/preferences-schema';
import { useAIModel, useAiProviderConfig, usePreferencesStore } from '@/store/preferences-store';
import type { Model } from '@/types';

import { buildModelOptions } from './build-options';

interface ModelSelectorProps {
  className?: string;
}

/**
 * Provider + model picker, driven by the provider registry (no hardcoded provider
 * list). Groups models by provider `kind`: Ollama (local server) and cloud lists
 * are fetched; CLI agents (Claude Code / Codex / Gemini CLI) contribute their
 * curated aliases when their binary is detected.
 */
export function ModelSelector({ className }: ModelSelectorProps) {
  const api = useAppClient();
  const { data: modelList = [] } = useAIModels();
  const ollamaModels = modelList as Model[];
  const aiModel = useAIModel();
  const setAIModel = usePreferencesStore((s) => s.setAIModel);
  const setProviderSettings = usePreferencesStore((s) => s.setProviderSettings);
  const setActiveProvider = usePreferencesStore((s) => s.setActiveProvider);
  const providerConfig = useAiProviderConfig();
  const activeProvider = providerConfig?.activeProvider ?? 'ollama';
  const activeProviderModel = providerConfig?.providers?.[activeProvider]?.model ?? '';

  // Cloud key + model status, driven off the registry (no hardcoded provider
  // list) so a new cloud provider — e.g. Ollama Cloud — is picked up with no
  // change here. Custom base URL only applies to the OpenAI-compatible provider.
  const cloudProviders = PROVIDER_ORDER.filter((p) => PROVIDERS[p].kind === 'cloud');
  const baseUrlFor = (p: AiProvider): string | undefined =>
    p === 'openai-compatible'
      ? providerConfig?.providers?.['openai-compatible']?.baseUrl
      : undefined;

  const keyQueries = useQueries({
    queries: cloudProviders.map((p) => ({
      queryKey: [...keys.ai.models, 'provider-key', p],
      queryFn: () => api.ai.hasProviderKey({ provider: p }),
      staleTime: 30_000,
    })),
  });
  const connected = new Map<AiProvider, boolean>(
    cloudProviders.map((p, i) => [p, keyQueries[i]?.data?.has ?? false])
  );

  const modelQueries = useQueries({
    queries: cloudProviders.map((p) => ({
      queryKey: [...keys.ai.models, 'provider-models', p, baseUrlFor(p) ?? ''],
      queryFn: () => api.ai.listProviderModels({ provider: p, baseUrl: baseUrlFor(p) }),
      enabled: connected.get(p) ?? false,
      staleTime: 300_000,
    })),
  });
  const cloudModelNames = new Map<AiProvider, string[]>(
    cloudProviders.map((p, i) => [
      p,
      ((modelQueries[i]?.data as Array<{ name: string }> | undefined) ?? []).map((m) => m.name),
    ])
  );

  // CLI-agent availability (binary detected) from the system health probe.
  const { data: health } = useSystemHealth();
  const cliDetected = (id: string) => health?.cliAgents?.[id]?.detected ?? false;

  // Grouped options, derived from the registry by provider kind (pure helper).
  const options = buildModelOptions(PROVIDER_ORDER, PROVIDERS, {
    ollamaModels: ollamaModels.map((m) => m.name),
    cliDetected,
    cloudConnected: (p) => connected.get(p) ?? false,
    cloudModels: (p) => cloudModelNames.get(p) ?? [],
  });

  // Current selection as "provider||model" (Ollama keeps its own defaultModel).
  const selectedValue =
    activeProvider === 'ollama'
      ? aiModel?.defaultModel
        ? `ollama||${aiModel.defaultModel}`
        : ''
      : activeProviderModel
        ? `${activeProvider}||${activeProviderModel}`
        : '';

  const handleModelChange = (value: string) => {
    const [provider, model] = value.split('||');
    if (!provider || !model) return;
    const p = provider as AiProvider;
    setProviderSettings(p, { model });
    if (p === 'ollama') {
      setAIModel({ defaultModel: model, temperature: 0.7, maxTokens: 2000 });
    }
    setActiveProvider(p);
  };

  return (
    <div className={className}>
      <div className="flex items-center gap-2">
        <div className="flex-1 min-w-0 w-full">
          <Dropdown
            options={options}
            value={selectedValue}
            onChange={handleModelChange}
            placeholder="Select a model…"
            icon={<Cpu size={13} />}
          />
        </div>
      </div>
    </div>
  );
}

export function useSelectedModel(): string {
  const aiModel = useAIModel();
  const providerConfig = useAiProviderConfig();
  const activeProvider = providerConfig?.activeProvider ?? 'ollama';
  const activeProviderModel = providerConfig?.providers?.[activeProvider]?.model ?? '';

  // Ollama uses its own defaultModel; every other provider stores its model in the
  // per-provider config (CLI agents may leave it empty → the tool's own default).
  const model = activeProvider === 'ollama' ? (aiModel?.defaultModel ?? '') : activeProviderModel;
  // Strip a provider prefix if one slipped in (e.g. "ollama||gpt-oss" -> "gpt-oss").
  return model.includes('||') ? (model.split('||')[1] ?? model) : model;
}

export function useSelectedProvider(): string {
  const providerConfig = useAiProviderConfig();
  return providerConfig?.activeProvider ?? 'ollama';
}

export function useCanUseAI(): { canUse: boolean; reason?: string } {
  const aiModel = useAIModel();
  const providerConfig = useAiProviderConfig();
  const activeProvider = providerConfig?.activeProvider ?? 'ollama';
  const kind = PROVIDERS[activeProvider]?.kind ?? 'local-server';
  const activeProviderModel = providerConfig?.providers?.[activeProvider]?.model ?? '';

  // Hooks must run unconditionally regardless of which branch applies below.
  const providerKeyQuery = useHasProviderKey(activeProvider);
  const { data: health } = useSystemHealth();

  if (kind === 'cloud') {
    if (!(providerKeyQuery.data?.has ?? false)) return { canUse: false, reason: 'addApiKey' };
    if (!activeProviderModel) return { canUse: false, reason: 'selectModel' };
    return { canUse: true };
  }
  if (kind === 'cli-agent') {
    // Keyless; model optional (empty → the tool's default). Only gate on the
    // binary being installed.
    const detected = health?.cliAgents?.[activeProvider]?.detected ?? false;
    return detected ? { canUse: true } : { canUse: false, reason: 'installCli' };
  }
  // local-server (Ollama)
  if (!aiModel?.defaultModel) return { canUse: false, reason: 'selectModel' };
  return { canUse: true };
}
