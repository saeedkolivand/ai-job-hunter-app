import { isOllamaFamily } from '@/lib/ai-providers/provider-meta';
import { useActiveConfig, useHasProviderKey } from '@/services';
import type { AiProvider } from '@/store/preferences-schema';

/**
 * True when the active AI provider is Ollama-family and the free Ollama
 * account key (`ollama-cloud`) is missing. Ollama has no model-side web
 * search, so company research (`ai_research_company`, the interview-questions
 * brief) silently returns `''` without this key — a missing key is otherwise
 * indistinguishable from "found nothing". Non-blocking signal only: callers
 * render a hint next to the affected control, never gate generation on it.
 *
 * Guards BOTH cold-boot windows rather than trust either query's fallback:
 * `useActiveConfig`'s `activeProvider` defaults to `'ollama'` before it
 * resolves (would flash the hint for every provider — the bug #682 fixed for
 * `StepFineTune`), and the `ollama-cloud` key query isn't boot-prefetched the
 * way `activeConfig` is, so its own pending window is just as reachable.
 * Suppress the hint while either is still pending.
 */
export function useNeedsResearchKey(): boolean {
  const { data: providerConfig, isPending: configPending } = useActiveConfig();
  const activeProvider = (providerConfig?.activeProvider ?? 'ollama') as AiProvider;
  const { data: ollamaKey, isPending: keyPending } = useHasProviderKey('ollama-cloud');
  if (configPending || keyPending) return false;
  return isOllamaFamily(activeProvider) && !(ollamaKey?.has ?? false);
}
