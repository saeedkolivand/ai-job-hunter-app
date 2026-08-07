import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { keys, queryClient } from '@/services/query-client';
import { usePreferencesStore } from '@/store/preferences-store';

import { _registerClient } from '../app-client';
import { createMockClient } from '../mock-client';
import { runAnalysis, scoreLabel, verdictGradient } from './resume-ai';

describe('scoreLabel', () => {
  it('labels each score band', () => {
    expect(scoreLabel(95).label).toBe('Exceptional');
    expect(scoreLabel(80).label).toBe('Strong');
    expect(scoreLabel(65).label).toBe('Moderate');
    expect(scoreLabel(50).label).toBe('Weak');
    expect(scoreLabel(20).label).toBe('Poor Match');
  });
});

describe('verdictGradient', () => {
  it('returns a gradient per band', () => {
    expect(verdictGradient(85)).toContain('emerald');
    expect(verdictGradient(70)).toContain('blue');
    expect(verdictGradient(55)).toContain('yellow');
    expect(verdictGradient(30)).toContain('red');
  });
});

describe('runAnalysis', () => {
  let streamHandler: ((chunk: unknown) => void) | null = null;

  beforeEach(() => {
    vi.useFakeTimers();
    streamHandler = null;
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
    usePreferencesStore.setState({ aiProviderConfig: undefined });
    queryClient.removeQueries({ queryKey: keys.ai.activeConfig });
  });

  function register() {
    const client = createMockClient({
      ai: {
        generate: vi.fn().mockResolvedValue({ jobId: 'job-1' }),
        onStream: vi.fn((h: (chunk: unknown) => void) => {
          streamHandler = h;
          return () => {};
        }),
      },
      jobs: { get: vi.fn().mockResolvedValue(null), cancel: vi.fn() },
    });
    _registerClient(client);
    return client;
  }

  async function flushUntilStreaming() {
    for (let i = 0; i < 5 && !streamHandler; i++) await Promise.resolve();
  }

  it('streams, collects and validates the analysis JSON', async () => {
    register();
    const onToken = vi.fn();
    const promise = runAnalysis({
      resume: 'Professional Summary\nExperienced engineer.',
      jobAd: 'We want a React engineer.',
      model: 'llama3',
      onToken,
    });
    await flushUntilStreaming();
    expect(streamHandler).toBeTypeOf('function');

    streamHandler?.({ jobId: 'job-1', delta: '{"scores":{"ats":72}}', done: false });
    streamHandler?.({ jobId: 'job-1', done: true });

    const result = await promise;
    expect(result.scores.ats).toBe(72);
    // The shared <think> splitter may chunk the streamed content (it holds back a
    // few trailing chars to detect split tags), so assert the reassembled stream.
    expect(onToken.mock.calls.map((c) => c[0]).join('')).toBe('{"scores":{"ats":72}}');
    expect(result.detectedLanguages).toHaveProperty('mismatch');
  });

  it('rejects when the model returns unparseable output', async () => {
    register();
    const promise = runAnalysis({ resume: 'r', jobAd: 'j', model: 'llama3' });
    await flushUntilStreaming();
    streamHandler?.({ jobId: 'job-1', delta: 'totally not json', done: true });
    await expect(promise).rejects.toThrow(/malformed/);
  });

  it('sends the deterministic intent and no default temperature', async () => {
    const client = register();
    const promise = runAnalysis({ resume: 'r', jobAd: 'j', model: 'llama3' });
    await flushUntilStreaming();
    streamHandler?.({ jobId: 'job-1', delta: '{"scores":{"ats":50}}', done: true });
    await promise;

    const call = (client.ai.generate as ReturnType<typeof vi.fn>).mock.calls[0];
    const arg = call?.[0] as { temperature?: number; intent?: string };
    expect(arg.intent).toBe('deterministic');
    // No Ollama override configured — must send nothing, never a hardcoded
    // default (the historical `0.1` bare literal this test guards against).
    expect(arg.temperature).toBeUndefined();
  });

  it('forwards the Settings analysis temperature slider — previously a silent no-op', async () => {
    // Regression: this call used a bare `temperature: 0.1` literal that never
    // went through `resolveTemperature`, so the Settings "analysis" slider
    // never actually reached the analysis it is named for.
    queryClient.setQueryData(keys.ai.activeConfig, {
      activeProvider: 'ollama',
      model: 'llama3',
      providers: { ollama: { model: 'llama3' } },
    });
    usePreferencesStore.setState({
      aiProviderConfig: {
        activeProvider: 'ollama',
        providers: {
          ollama: { model: 'llama3', modelLimits: { llama3: { temperature: { analysis: 0.22 } } } },
        },
      },
    });
    const client = register();
    const promise = runAnalysis({ resume: 'r', jobAd: 'j', model: 'llama3' });
    await flushUntilStreaming();
    streamHandler?.({ jobId: 'job-1', delta: '{"scores":{"ats":50}}', done: true });
    await promise;

    const call = (client.ai.generate as ReturnType<typeof vi.fn>).mock.calls[0];
    const arg = call?.[0] as { temperature?: number };
    expect(arg.temperature).toBeCloseTo(0.22);
  });
});
