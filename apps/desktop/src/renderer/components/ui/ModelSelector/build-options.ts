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
  /** Whether a cloud provider has a stored key. */
  cloudConnected: (p: AiProvider) => boolean;
  /** Live cloud model names fetched from the provider (may be empty). */
  cloudModels: (p: AiProvider) => string[];
}

/**
 * Build the grouped `provider||model` options for the model picker — purely from
 * the registry + the supplied sources, so adding a provider needs **no change
 * here** (this is what keeps the picker registry-driven). A connected cloud
 * provider falls back to its curated `meta.models` when its live model list is
 * empty (e.g. Ollama Cloud before `/v1/models` is reachable), so it always
 * offers something to pick.
 */
export function buildModelOptions(
  order: AiProvider[],
  meta: Record<AiProvider, ProviderMeta>,
  sources: ModelSources
): ModelOption[] {
  return order.flatMap((p) => {
    const m = meta[p];
    let names: string[];
    if (m.kind === 'local-server') {
      names = sources.ollamaModels;
    } else if (m.kind === 'cli-agent') {
      names = sources.cliDetected(p) ? m.models : [];
    } else if (!sources.cloudConnected(p)) {
      names = [];
    } else {
      const live = sources.cloudModels(p);
      names = live.length > 0 ? live : m.models;
    }
    return names.map((name) => ({ value: `${p}||${name}`, label: name, section: m.label }));
  });
}
