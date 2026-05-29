import { Radio } from 'lucide-react';

import { PROVIDERS } from '@/lib/ai-providers/provider-meta';
import type { AiProvider } from '@/store/preferences-schema';
import { useAiProviderConfig } from '@/store/preferences-store';

/**
 * Surfaces the *actual* routing the backend will use — active provider, model,
 * and chat endpoint — so provider/model routing bugs are obvious at a glance.
 * The endpoints mirror the backend provider clients (commands/ai_provider/*).
 */
function chatEndpoint(provider: string, model: string, baseUrl?: string): string {
  // CLI agents shell out to a local binary — there's no HTTP endpoint to show.
  if (PROVIDERS[provider as AiProvider]?.kind === 'cli-agent') return `${provider} (cli)`;
  switch (provider) {
    case 'ollama':
      return 'http://127.0.0.1:11434/api/chat';
    case 'openai':
      return 'https://api.openai.com/v1/chat/completions';
    case 'openai-compatible':
      return `${baseUrl?.trim() || 'https://api.openai.com/v1'}/chat/completions`;
    case 'anthropic':
      return 'https://api.anthropic.com/v1/messages';
    case 'gemini':
      return `https://generativelanguage.googleapis.com/v1beta/models/${model || '<model>'}:streamGenerateContent`;
    default:
      return '—';
  }
}

export function ProviderDebugBadge() {
  const config = useAiProviderConfig();
  const provider = config?.activeProvider ?? 'ollama';
  const settings = config?.providers?.[provider];
  const model = settings?.model ?? '';
  const baseUrl = settings?.baseUrl;
  const endpoint = chatEndpoint(provider, model, baseUrl);

  return (
    <div className="flex items-start gap-2 rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2 font-mono text-[10px] leading-relaxed text-foreground/45">
      <Radio size={11} className="mt-0.5 shrink-0 text-brand-soft/70" />
      <div className="min-w-0 break-all">
        <span className="text-foreground/70">provider</span>={provider}
        {'  ·  '}
        <span className="text-foreground/70">model</span>={model || '—'}
        {'  ·  '}
        <span className="text-foreground/70">endpoint</span>={endpoint}
      </div>
    </div>
  );
}
