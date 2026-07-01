import { describe, expect, it } from 'vitest';

import type { ProviderMeta } from '@/lib/ai-providers/provider-meta';
import type { AiProvider } from '@/store/preferences-schema';

import { buildModelOptions, type ModelSources } from './build-options';

function meta(kind: ProviderMeta['kind'], label: string, models: string[]): ProviderMeta {
  return { kind, label, models, description: '', docsUrl: '', color: '' };
}

const META = {
  ollama: meta('local-server', 'Ollama (Local)', []),
  'ollama-cloud': meta('cloud', 'Ollama Cloud', ['gpt-oss:120b', 'gpt-oss:20b']),
  openai: meta('cloud', 'OpenAI', ['gpt-4o']),
  'claude-code': meta('cli-agent', 'Claude Code', ['sonnet', 'opus']),
} as unknown as Record<AiProvider, ProviderMeta>;

const baseSources: ModelSources = {
  ollamaModels: [],
  cliDetected: () => false,
  cloudConnected: () => false,
  cloudModels: () => [],
};

describe('buildModelOptions', () => {
  it('includes Ollama Cloud when connected — falling back to curated models when the live list is empty', () => {
    const options = buildModelOptions(['ollama-cloud'], META, {
      ...baseSources,
      cloudConnected: (p) => p === 'ollama-cloud',
      cloudModels: () => [], // live /v1/models returned nothing yet
    });
    expect(options.map((o) => o.value)).toEqual([
      'ollama-cloud||gpt-oss:120b',
      'ollama-cloud||gpt-oss:20b',
    ]);
    expect(options[0]?.section).toBe('Ollama Cloud');
  });

  it('prefers the live cloud model list over the curated fallback', () => {
    const options = buildModelOptions(['ollama-cloud'], META, {
      ...baseSources,
      cloudConnected: () => true,
      cloudModels: () => ['deepseek-v3.1:671b'],
    });
    expect(options.map((o) => o.label)).toEqual(['deepseek-v3.1:671b']);
  });

  it('omits a cloud provider entirely when it is not connected', () => {
    const options = buildModelOptions(['ollama-cloud', 'openai'], META, baseSources);
    expect(options).toHaveLength(0);
  });

  it('groups local Ollama and detected CLI agents by their registry kind', () => {
    const options = buildModelOptions(['ollama', 'claude-code'], META, {
      ...baseSources,
      ollamaModels: ['llama3.2:1b'],
      cliDetected: (p) => p === 'claude-code',
    });
    expect(options).toEqual([
      { value: 'ollama||llama3.2:1b', label: 'llama3.2:1b', section: 'Ollama (Local)' },
      { value: 'claude-code||sonnet', label: 'sonnet', section: 'Claude Code' },
      { value: 'claude-code||opus', label: 'opus', section: 'Claude Code' },
    ]);
  });

  it('hides CLI-agent models until the binary is detected', () => {
    const options = buildModelOptions(['claude-code'], META, baseSources);
    expect(options).toHaveLength(0);
  });
});
