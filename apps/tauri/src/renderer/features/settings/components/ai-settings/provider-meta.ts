import type { AiProvider } from '@/store/preferences-schema';

export interface ProviderMeta {
  label: string;
  description: string;
  docsUrl: string;
  color: string;
  models: string[];
}

export const PROVIDERS: Record<AiProvider, ProviderMeta> = {
  ollama: {
    label: 'Ollama (Local)',
    description: 'Run models locally — no API key, no cloud, fully private.',
    docsUrl: 'https://ollama.com',
    color: 'text-emerald-400',
    models: [],
  },
  openai: {
    label: 'OpenAI',
    description: 'GPT-4o, GPT-4 Turbo, and more via the OpenAI API.',
    docsUrl: 'https://platform.openai.com/api-keys',
    color: 'text-green-400',
    models: ['gpt-4o', 'gpt-4o-mini', 'gpt-4-turbo', 'gpt-3.5-turbo', 'o1', 'o1-mini'],
  },
  anthropic: {
    label: 'Anthropic (Claude)',
    description: 'Claude Opus, Sonnet, and Haiku via the Anthropic API.',
    docsUrl: 'https://console.anthropic.com/settings/keys',
    color: 'text-orange-400',
    models: ['claude-opus-4-7', 'claude-sonnet-4-6', 'claude-haiku-4-5-20251001'],
  },
  gemini: {
    label: 'Google Gemini',
    description: 'Gemini 2.0 Flash and Gemini 1.5 Pro via the Gemini API.',
    docsUrl: 'https://aistudio.google.com/app/apikey',
    color: 'text-blue-400',
    models: ['gemini-2.0-flash', 'gemini-1.5-pro', 'gemini-1.5-flash'],
  },
  'openai-compatible': {
    label: 'OpenAI-Compatible',
    description: 'Any server that speaks the OpenAI API: Groq, Together, LM Studio, etc.',
    docsUrl: 'https://platform.openai.com/docs/api-reference',
    color: 'text-purple-400',
    models: [],
  },
};

export const PROVIDER_ORDER: AiProvider[] = [
  'ollama',
  'openai',
  'anthropic',
  'gemini',
  'openai-compatible',
];
