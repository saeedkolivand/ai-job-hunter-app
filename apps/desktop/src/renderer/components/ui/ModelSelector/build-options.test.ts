import { describe, expect, it } from 'vitest';

import type { ProviderMeta } from '@/lib/ai-providers/provider-meta';
import type { AiProvider } from '@/store/preferences-schema';

import { buildModelOptions, type ModelSources } from './build-options';

function meta(kind: ProviderMeta['kind'], label: string, models: string[]): ProviderMeta {
  return { kind, label, models, description: '', docsUrl: '', color: '' };
}

const META = {
  ollama: meta('local-server', 'Ollama (Local)', []),
  'ollama-cloud': meta('cloud', 'Ollama Cloud', []),
  openai: meta('cloud', 'OpenAI', []),
  'claude-code': meta('cli-agent', 'Claude Code', ['sonnet', 'opus']),
} as unknown as Record<AiProvider, ProviderMeta>;

const baseSources: ModelSources = {
  ollamaModels: [],
  cliDetected: () => false,
  cloudConnected: () => false,
  cloudModels: () => [],
};

describe('buildModelOptions', () => {
  it('lists a connected cloud provider from its live/cached model entries', () => {
    const options = buildModelOptions(['ollama-cloud'], META, {
      ...baseSources,
      cloudConnected: (p) => p === 'ollama-cloud',
      cloudModels: () => [{ name: 'gpt-oss:120b' }, { name: 'gpt-oss:20b' }],
    });
    expect(options.map((o) => o.value)).toEqual([
      'ollama-cloud||gpt-oss:120b',
      'ollama-cloud||gpt-oss:20b',
    ]);
    expect(options[0]?.section).toBe('Ollama Cloud');
  });

  it('labels a cloud option with displayName when the provider returns one, else name', () => {
    const options = buildModelOptions(['openai'], META, {
      ...baseSources,
      cloudConnected: () => true,
      cloudModels: () => [{ name: 'gpt-4o', displayName: 'GPT-4o' }, { name: 'o1' }],
    });
    expect(options).toEqual([
      { value: 'openai||gpt-4o', label: 'GPT-4o', section: 'OpenAI' },
      { value: 'openai||o1', label: 'o1', section: 'OpenAI' },
    ]);
  });

  it('sorts cloud options newest-first by createdAt', () => {
    const options = buildModelOptions(['openai'], META, {
      ...baseSources,
      cloudConnected: () => true,
      cloudModels: () => [
        { name: 'old', createdAt: 1_000 },
        { name: 'new', createdAt: 3_000 },
        { name: 'mid', createdAt: 2_000 },
      ],
    });
    expect(options.map((o) => o.value)).toEqual(['openai||new', 'openai||mid', 'openai||old']);
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
