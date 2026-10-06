import { describe, expect, it, vi } from 'vitest';

import { usePreferencesStore } from '@/store/preferences-store';

import { computeStreamTimeoutMs } from '../stream-promise';
import {
  extractMetadata,
  generateCoverLetter,
  generateResume,
  resolveRewriteTimeoutMs,
} from './generation';
import { installGenerationHooks, register, setActive, streamThrough } from './test-support';

installGenerationHooks();

describe('local model limits wiring', () => {
  it('sends the per-model contextWindow + maxTokens on the ollama path', async () => {
    setActive('ollama', 'llama3');
    usePreferencesStore.setState({
      aiProviderConfig: {
        activeProvider: 'ollama',
        providers: {
          ollama: {
            model: 'llama3',
            modelLimits: { llama3: { contextWindow: 16384, maxTokens: 4096 } },
          },
        },
      },
    });
    const client = register();
    await streamThrough(extractMetadata('resume', 'job ad', 'llama3'), '{}');

    // `provider` is no longer on the wire (backend-owned, task #16); only the local
    // tuning knobs (contextWindow/maxTokens) are sent.
    expect(client.ai.generatePipeline).toHaveBeenCalledWith(
      expect.objectContaining({ contextWindow: 16384, maxTokens: 4096 })
    );
  });

  it('omits the local context window for cloud providers', async () => {
    // Active provider is the backend store; the ollama limits below must NOT leak
    // into a cloud generation.
    setActive('openai', 'gpt-4o');
    usePreferencesStore.setState({
      aiProviderConfig: {
        activeProvider: 'openai',
        providers: {
          openai: { model: 'gpt-4o' },
          ollama: {
            model: 'llama3',
            modelLimits: { llama3: { contextWindow: 16384, maxTokens: 4096 } },
          },
        },
      },
    });
    const client = register();
    await streamThrough(extractMetadata('resume', 'job ad', 'gpt-4o'), '{}');

    const call = (client.ai.generatePipeline as ReturnType<typeof vi.fn>).mock.calls[0];
    expect(call).toBeDefined();
    const arg = call?.[0] as { provider?: string; contextWindow?: number };
    // Routing no longer crosses the wire, and the cloud path sends no local ctx.
    expect(arg.provider).toBeUndefined();
    expect(arg.contextWindow).toBeUndefined();
  });
});

describe('resolveRewriteTimeoutMs', () => {
  it.each(['low', 'high', 'xhigh'] as const)(
    "equals computeStreamTimeoutMs for the active provider's resolved effort (%s)",
    (effort) => {
      setActive('ollama', 'llama3');
      usePreferencesStore.setState({
        aiProviderConfig: {
          activeProvider: 'ollama',
          providers: { ollama: { model: 'llama3', effort } },
        },
      });

      expect(resolveRewriteTimeoutMs('llama3')).toBe(computeStreamTimeoutMs(effort));
    }
  );

  it('falls back to the flat baseline when the active provider has no configured effort', () => {
    setActive('ollama', 'llama3');

    expect(resolveRewriteTimeoutMs('llama3')).toBe(computeStreamTimeoutMs(undefined));
  });
});

describe('per-step temperature override (Ollama-only user-chosen value)', () => {
  // The renderer no longer ships a default sampling number for any step —
  // each provider adapter picks its own (or none) per (model, intent), see
  // `AiProvider::sampling_profile` (`commands/ai_provider/mod.rs`). Only the
  // Ollama per-model/per-step temperature slider (`LocalModelLimits.tsx`) is
  // forwarded as an explicit `temperature`; everything else rides on `intent`.
  const META = {
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    mismatch: false,
    candidateName: 'X',
    jobTitle: 'Y',
    companyName: 'Z',
    targetLanguage: 'en',
    topRequirements: [],
  };

  const argOf = (client: ReturnType<typeof register>) => {
    const call = (client.ai.generatePipeline as ReturnType<typeof vi.fn>).mock.calls[0];
    return call?.[0] as { temperature?: number; intent?: string };
  };

  const setOllama = (temperature?: Record<string, number>) => {
    setActive('ollama', 'llama3');
    usePreferencesStore.setState({
      aiProviderConfig: {
        activeProvider: 'ollama',
        providers: { ollama: { model: 'llama3', modelLimits: { llama3: { temperature } } } },
      },
    });
  };

  it('applies the per-step override to its own step only (cover set, resume unset)', async () => {
    setOllama({ cover: 0.85 });
    const client = register();
    await streamThrough(
      generateCoverLetter('My resume', 'Job ad', META, 'recruiter', 'llama3', vi.fn()),
      'Dear Hiring Team.'
    );
    expect(argOf(client).temperature).toBeCloseTo(0.85);
    expect(argOf(client).intent).toBe('prose_grounded');
  });

  it('sends no temperature when that step has no override — the adapter decides', async () => {
    // Only `cover` is set — résumé generation must still send nothing.
    setOllama({ cover: 0.85 });
    const client = register();
    await streamThrough(
      generateResume('My resume', 'Job ad', META, 'ats', 'llama3', vi.fn()),
      'RESUME CONTENT'
    );
    expect(argOf(client).temperature).toBeUndefined();
    expect(argOf(client).intent).toBe('deterministic');
  });

  it('ignores the override for non-ollama providers (temperature stays unset)', async () => {
    // Cloud provider active, but ollama still carries a cover override: must
    // never leak across providers.
    setActive('openai', 'gpt-4o');
    usePreferencesStore.setState({
      aiProviderConfig: {
        activeProvider: 'openai',
        providers: {
          openai: { model: 'gpt-4o' },
          ollama: { model: 'llama3', modelLimits: { llama3: { temperature: { cover: 0.85 } } } },
        },
      },
    });
    const client = register();
    await streamThrough(
      generateCoverLetter('My resume', 'Job ad', META, 'recruiter', 'gpt-4o', vi.fn()),
      'Dear Hiring Team.'
    );
    expect(argOf(client).temperature).toBeUndefined();
  });
});
