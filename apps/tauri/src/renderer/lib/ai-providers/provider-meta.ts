import type { AiProvider } from '@/store/preferences-schema';

/**
 * How a provider is configured & authenticated — the single discriminator the UI
 * branches on (never on specific provider ids):
 * - `cloud`        — HTTP API with a key stored in the OS keychain.
 * - `local-server` — a local server (Ollama); detected via a health probe.
 * - `cli-agent`    — a locally-installed headless CLI (Claude Code, …); keyless,
 *                    uses its own login, detected by the presence of its binary.
 */
export type ProviderKind = 'cloud' | 'local-server' | 'cli-agent';

export interface ProviderMeta {
  kind: ProviderKind;
  label: string;
  description: string;
  docsUrl: string;
  color: string;
  models: string[];
  /** Reasoning-effort levels this provider supports (CLI agents only, e.g. Codex). */
  efforts?: string[];
}

export const PROVIDERS: Record<AiProvider, ProviderMeta> = {
  ollama: {
    kind: 'local-server',
    label: 'Ollama (Local)',
    description: 'Run models locally — no API key, no cloud, fully private.',
    docsUrl: 'https://ollama.com',
    color: 'text-emerald-400',
    models: [],
  },
  'ollama-cloud': {
    kind: 'cloud',
    label: 'Ollama Cloud',
    description:
      'Run large hosted Ollama models with a free Ollama key — also powers company research.',
    docsUrl: 'https://ollama.com/settings/keys',
    color: 'text-emerald-400',
    models: ['gpt-oss:120b', 'gpt-oss:20b', 'deepseek-v3.1:671b', 'qwen3-coder:480b'],
  },
  openai: {
    kind: 'cloud',
    label: 'OpenAI',
    description: 'GPT-4o, GPT-4 Turbo, and more via the OpenAI API.',
    docsUrl: 'https://platform.openai.com/api-keys',
    color: 'text-green-400',
    models: ['gpt-4o', 'gpt-4o-mini', 'gpt-4-turbo', 'gpt-3.5-turbo', 'o1', 'o1-mini'],
  },
  anthropic: {
    kind: 'cloud',
    label: 'Anthropic (Claude)',
    description: 'Claude Opus, Sonnet, and Haiku via the Anthropic API.',
    docsUrl: 'https://console.anthropic.com/settings/keys',
    color: 'text-orange-400',
    models: ['claude-opus-4-7', 'claude-sonnet-4-6', 'claude-haiku-4-5-20251001'],
  },
  gemini: {
    kind: 'cloud',
    label: 'Google Gemini',
    description: 'Gemini 2.0 Flash and Gemini 1.5 Pro via the Gemini API.',
    docsUrl: 'https://aistudio.google.com/app/apikey',
    color: 'text-blue-400',
    models: ['gemini-2.0-flash', 'gemini-1.5-pro', 'gemini-1.5-flash'],
  },
  'openai-compatible': {
    kind: 'cloud',
    label: 'OpenAI-Compatible',
    description: 'Any server that speaks the OpenAI API: Groq, Together, LM Studio, etc.',
    docsUrl: 'https://platform.openai.com/docs/api-reference',
    color: 'text-purple-400',
    models: [],
  },
  'claude-code': {
    kind: 'cli-agent',
    label: 'Claude Code',
    description: 'Use your installed Claude Code CLI — your existing Claude login, no API key.',
    docsUrl: 'https://docs.anthropic.com/en/docs/claude-code',
    color: 'text-orange-400',
    models: ['sonnet', 'opus', 'haiku'],
  },
  codex: {
    kind: 'cli-agent',
    label: 'OpenAI Codex',
    description: 'Use your installed Codex CLI — your existing ChatGPT login, no API key.',
    docsUrl: 'https://developers.openai.com/codex/cli',
    color: 'text-green-400',
    models: ['gpt-5-codex', 'o4-mini'],
    efforts: ['low', 'medium', 'high'],
  },
  'gemini-cli': {
    kind: 'cli-agent',
    label: 'Gemini CLI',
    description: 'Use your installed Gemini CLI — your existing Google login, no API key.',
    docsUrl: 'https://github.com/google-gemini/gemini-cli',
    color: 'text-blue-400',
    models: ['gemini-2.5-pro', 'gemini-2.5-flash'],
  },
};

export const PROVIDER_ORDER: AiProvider[] = [
  'ollama',
  'ollama-cloud',
  'openai',
  'anthropic',
  'gemini',
  'openai-compatible',
  'claude-code',
  'codex',
  'gemini-cli',
];

/**
 * Ollama-family providers (local + cloud). They share the Ollama account key
 * (`ai:ollama-cloud`) and use the Ollama Web Search API for company research —
 * so unlike other providers they need that key before research can run.
 */
export function isOllamaFamily(provider: AiProvider): boolean {
  return provider === 'ollama' || provider === 'ollama-cloud';
}
