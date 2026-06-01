import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { usePreferencesStore } from '@/store/preferences-store';

import { _registerClient } from '../../app-client';
import { createMockClient } from '../../mock-client';
import { extractMetadata, generateCoverLetter, generateResume } from './generation';

let streamHandler: ((chunk: unknown) => void) | null = null;

function register() {
  const client = createMockClient({
    ai: {
      generatePipeline: vi.fn().mockResolvedValue({ jobId: 'gen-1' }),
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
  for (let i = 0; i < 6 && !streamHandler; i++) await Promise.resolve();
}

function emit(delta: string) {
  streamHandler?.({ jobId: 'gen-1', delta, done: false });
}
function done() {
  streamHandler?.({ jobId: 'gen-1', done: true });
}

beforeEach(() => {
  vi.useFakeTimers();
  streamHandler = null;
});
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  usePreferencesStore.setState({ aiProviderConfig: undefined });
});

describe('extractMetadata', () => {
  it('parses streamed metadata JSON and overrides languages client-side', async () => {
    register();
    const p = extractMetadata('Professional Summary\nJane Smith resume', 'A React role', 'llama3');
    await flushUntilStreaming();
    emit('{"candidateName":"Jane Smith","jobTitle":"Frontend Engineer","companyName":"Acme"}');
    done();
    const meta = await p;
    expect(meta.candidateName).toBe('Jane Smith');
    expect(meta.jobTitle).toBe('Frontend Engineer');
    expect(meta).toHaveProperty('mismatch');
  });

  it('falls back to regex extraction when the model returns no JSON', async () => {
    register();
    const resume = 'John Doe\nsoftware engineer with 10 years experience.';
    const jobAd = 'Position: Senior Engineer\nCompany: Acme';
    const p = extractMetadata(resume, jobAd, 'llama3');
    await flushUntilStreaming();
    emit('sorry, no json available');
    done();
    const meta = await p;
    expect(meta.candidateName).toBe('John Doe');
    expect(meta.jobTitle).toBe('Senior Engineer');
    expect(meta.companyName).toBe('Acme');
  });
});

describe('generateResume', () => {
  it('streams content, strips <think> blocks, and forwards tokens', async () => {
    register();
    const onToken = vi.fn();
    const onThinking = vi.fn();
    const p = generateResume(
      'My resume',
      'Job ad',
      {
        resumeLanguage: 'en',
        jobAdLanguage: 'en',
        mismatch: false,
        candidateName: 'X',
        jobTitle: 'Y',
        companyName: 'Z',
        targetLanguage: 'en',
        topRequirements: [],
      },
      'ats',
      'llama3',
      onToken,
      'en',
      undefined,
      onThinking
    );
    await flushUntilStreaming();
    emit('<think>reasoning here</think>VISIBLE RESUME CONTENT');
    done();
    const out = await p;
    expect(out).toContain('VISIBLE RESUME CONTENT');
    expect(out).not.toContain('reasoning here');
    expect(onThinking).toHaveBeenCalled();
    expect(onToken).toHaveBeenCalled();
  });
});

describe('generateCoverLetter', () => {
  it('returns the cleaned letter text', async () => {
    register();
    const onToken = vi.fn();
    const p = generateCoverLetter(
      'My resume',
      'Job ad',
      {
        resumeLanguage: 'en',
        jobAdLanguage: 'en',
        mismatch: false,
        candidateName: 'X',
        jobTitle: 'Y',
        companyName: 'Z',
        targetLanguage: 'en',
        topRequirements: [],
      },
      'recruiter',
      'llama3',
      onToken
    );
    await flushUntilStreaming();
    emit('Dear Hiring Team, I am a great fit for this role and more.');
    done();
    const out = await p;
    expect(out).toContain('Dear Hiring Team');
  });
});

describe('local model limits wiring', () => {
  it('sends the per-model contextWindow + maxTokens on the ollama path', async () => {
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
    const p = extractMetadata('resume', 'job ad', 'llama3');
    await flushUntilStreaming();
    emit('{}');
    done();
    await p;

    expect(client.ai.generatePipeline).toHaveBeenCalledWith(
      expect.objectContaining({ provider: 'ollama', contextWindow: 16384, maxTokens: 4096 })
    );
  });

  it('omits the local context window for cloud providers', async () => {
    usePreferencesStore.setState({
      aiProviderConfig: {
        activeProvider: 'openai',
        providers: {
          openai: { model: 'gpt-4o' },
          // Even with ollama limits stored, the cloud path must not send them.
          ollama: {
            model: 'llama3',
            modelLimits: { llama3: { contextWindow: 16384, maxTokens: 4096 } },
          },
        },
      },
    });
    const client = register();
    const p = extractMetadata('resume', 'job ad', 'gpt-4o');
    await flushUntilStreaming();
    emit('{}');
    done();
    await p;

    const call = (client.ai.generatePipeline as ReturnType<typeof vi.fn>).mock.calls[0];
    expect(call).toBeDefined();
    const arg = call?.[0] as { provider: string; contextWindow?: number };
    expect(arg.provider).toBe('openai');
    expect(arg.contextWindow).toBeUndefined();
  });
});
