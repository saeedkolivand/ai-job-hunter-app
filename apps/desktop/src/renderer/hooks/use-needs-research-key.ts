import { useSelectedProvider } from '@/components/ui/ModelSelector';
import { isOllamaFamily } from '@/lib/ai-providers/provider-meta';
import { useHasProviderKey } from '@/services';
import type { AiProvider } from '@/store/preferences-schema';

/**
 * True when the active AI provider is Ollama-family and the free Ollama
 * account key (`ollama-cloud`) is missing. Ollama has no model-side web
 * search, so company research (`ai_research_company`, the interview-questions
 * brief) silently returns `''` without this key — a missing key is otherwise
 * indistinguishable from "found nothing". Non-blocking signal only: callers
 * render a hint next to the affected control, never gate generation on it.
 */
export function useNeedsResearchKey(): boolean {
  const activeProvider = useSelectedProvider();
  const { data: ollamaKey } = useHasProviderKey('ollama-cloud');
  return isOllamaFamily(activeProvider as AiProvider) && !(ollamaKey?.has ?? false);
}
