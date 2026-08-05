import type { ProviderModelInfo } from '@ajh/shared';

import { sortModelsNewestFirst } from '@/lib/ai-providers/model-sort';
import type { ProviderMeta } from '@/lib/ai-providers/provider-meta';
import type { AiProvider } from '@/store/preferences-schema';

export interface ModelOption {
  /** Encoded as `provider||model` so the picker carries both. */
  value: string;
  label: string;
  /** Group header — the provider's display label. */
  section: string;
}

export interface ModelSources {
  /** Local Ollama model names (the local server's installed models). */
  ollamaModels: string[];
  /** Whether a CLI agent's binary is detected. */
  cliDetected: (p: AiProvider) => boolean;
  /** Whether a cloud provider can be fetched — a stored key, or (for the one
   *  keyless-capable provider, `openai-compatible`) always. */
  cloudConnected: (p: AiProvider) => boolean;
  /** Live (or last-known-good cached) cloud model entries — metadata-aware,
   *  since the backend catalogue carries `displayName`/`createdAt`. */
  cloudModels: (p: AiProvider) => ProviderModelInfo[];
}

/**
 * Build the grouped `provider||model` options for the model picker — purely from
 * the registry + the supplied sources, so adding a provider needs **no change
 * here** (this is what keeps the picker registry-driven).
 */
export function buildModelOptions(
  order: AiProvider[],
  meta: Record<AiProvider, ProviderMeta>,
  sources: ModelSources
): ModelOption[] {
  return order.flatMap((p) => {
    const m = meta[p];
    if (m.kind === 'local-server') {
      return sources.ollamaModels.map((name) => ({
        value: `${p}||${name}`,
        label: name,
        section: m.label,
      }));
    }
    if (m.kind === 'cli-agent') {
      const names = sources.cliDetected(p) ? m.models : [];
      return names.map((name) => ({ value: `${p}||${name}`, label: name, section: m.label }));
    }
    // cloud
    if (!sources.cloudConnected(p)) return [];
    return sortModelsNewestFirst(sources.cloudModels(p)).map((model) => ({
      value: `${p}||${model.name}`,
      label: model.displayName ?? model.name,
      section: m.label,
    }));
  });
}
