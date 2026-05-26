import { Cpu } from 'lucide-react';

import { Dropdown } from '@ajh/ui';

import { useAIModels, useHasProviderKey, useListProviderModels } from '@/services';
import { useAIModel, useAiProviderConfig, usePreferencesStore } from '@/store/preferences-store';
import type { Model } from '@/types';

interface ModelSelectorProps {
  className?: string;
}

const PROVIDER_LABELS: Record<string, string> = {
  ollama: 'Ollama (Local)',
  openai: 'OpenAI',
  anthropic: 'Anthropic',
  gemini: 'Google Gemini',
  'openai-compatible': 'OpenAI-Compatible',
};

export function ModelSelector({ className }: ModelSelectorProps) {
  const { data: modelList = [] } = useAIModels();
  const ollamaModels = modelList as Model[];
  const aiModel = useAIModel();
  const setAIModel = usePreferencesStore((s) => s.setAIModel);
  const setProviderSettings = usePreferencesStore((s) => s.setProviderSettings);
  const setActiveProvider = usePreferencesStore((s) => s.setAiProviderConfig);
  const providerConfig = useAiProviderConfig();
  const activeProvider = providerConfig?.activeProvider ?? 'ollama';
  const activeProviderModel = providerConfig?.providers?.[activeProvider]?.model ?? '';

  // Check connection status for all providers
  const openaiKey = useHasProviderKey('openai');
  const anthropicKey = useHasProviderKey('anthropic');
  const geminiKey = useHasProviderKey('gemini');
  const compatKey = useHasProviderKey('openai-compatible');

  const providerStatus: Record<string, boolean> = {
    ollama: true, // Ollama is always available (may be offline but we show it)
    openai: openaiKey.data?.has ?? false,
    anthropic: anthropicKey.data?.has ?? false,
    gemini: geminiKey.data?.has ?? false,
    'openai-compatible': compatKey.data?.has ?? false,
  };

  // Fetch models for all connected cloud providers
  const openaiModels = useListProviderModels('openai', providerStatus.openai);
  const anthropicModels = useListProviderModels('anthropic', providerStatus.anthropic);
  const geminiModels = useListProviderModels('gemini', providerStatus.gemini);
  const compatModels = useListProviderModels(
    'openai-compatible',
    providerStatus['openai-compatible']
  );

  // Build grouped options with sections
  const options = [
    // Ollama models
    ...ollamaModels.map((m) => ({
      value: `ollama||${m.name}`,
      label: m.name,
      section: PROVIDER_LABELS.ollama,
    })),
    // OpenAI models
    ...(openaiModels.data || []).map((m: { name: string }) => ({
      value: `openai||${m.name}`,
      label: m.name,
      section: PROVIDER_LABELS.openai,
    })),
    // Anthropic models
    ...(anthropicModels.data || []).map((m: { name: string }) => ({
      value: `anthropic||${m.name}`,
      label: m.name,
      section: PROVIDER_LABELS.anthropic,
    })),
    // Gemini models
    ...(geminiModels.data || []).map((m: { name: string }) => ({
      value: `gemini||${m.name}`,
      label: m.name,
      section: PROVIDER_LABELS.gemini,
    })),
    // OpenAI-compatible models
    ...(compatModels.data || []).map((m: { name: string }) => ({
      value: `openai-compatible||${m.name}`,
      label: m.name,
      section: PROVIDER_LABELS['openai-compatible'],
    })),
  ];

  // Current selected value in the format "provider||model"
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

    if (provider === 'ollama') {
      setAIModel({ defaultModel: model, temperature: 0.7, maxTokens: 2000 });
      setActiveProvider({ activeProvider: 'ollama', providers: providerConfig?.providers ?? {} });
    } else {
      setProviderSettings(provider as 'openai' | 'anthropic' | 'gemini' | 'openai-compatible', {
        model,
      });
      setActiveProvider({
        activeProvider: provider as 'openai' | 'anthropic' | 'gemini' | 'openai-compatible',
        providers: {
          ...providerConfig?.providers,
          [provider]: {
            ...(providerConfig?.providers?.[
              provider as 'openai' | 'anthropic' | 'gemini' | 'openai-compatible'
            ] ?? {}),
            model,
          },
        },
      });
    }
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
  const isCloudProvider = activeProvider !== 'ollama';
  const activeProviderModel = providerConfig?.providers?.[activeProvider]?.model ?? '';

  const model = isCloudProvider ? activeProviderModel : (aiModel?.defaultModel ?? '');
  // Strip provider prefix if present (e.g., "ollama||gpt-oss" -> "gpt-oss")
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
  const isCloudProvider = activeProvider !== 'ollama';
  const activeProviderModel = providerConfig?.providers?.[activeProvider]?.model ?? '';
  const providerKeyQuery = useHasProviderKey(activeProvider);
  const providerConnected = isCloudProvider ? (providerKeyQuery.data?.has ?? false) : true;

  if (isCloudProvider && !providerConnected) {
    return { canUse: false, reason: 'addApiKey' };
  }
  if (isCloudProvider && !activeProviderModel) {
    return { canUse: false, reason: 'selectModel' };
  }
  if (!isCloudProvider && !aiModel?.defaultModel) {
    return { canUse: false, reason: 'selectModel' };
  }
  return { canUse: true };
}
