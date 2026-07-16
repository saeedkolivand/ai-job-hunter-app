import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { GitHubRepo } from '@ajh/shared';

import { keys, queryClient } from '@/services/query-client';
import { usePreferencesStore } from '@/store/preferences-store';

import { _registerClient } from '../../app-client';
import { createMockClient } from '../../mock-client';
import {
  extractMetadata,
  generateApplicationAnswer,
  generateCoverLetter,
  generateGitHubProjects,
  generateResume,
  lookupSalaryRange,
  researchAnswer,
  researchCompany,
} from './generation';

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

// The active provider/model are backend-owned (task #16) and read imperatively via
// the singleton React Query cache; seed it directly. `modelLimits`/`effort` (tuning
// knobs) still come from Zustand `aiProviderConfig` (set via setState below).
function setActive(activeProvider: string, model: string) {
  queryClient.setQueryData(keys.ai.activeConfig, {
    activeProvider,
    model,
    providers: { [activeProvider]: { model } },
  });
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
  usePreferencesStore.setState({ aiProviderConfig: undefined, outputTone: 'professional' });
  queryClient.removeQueries({ queryKey: keys.ai.activeConfig });
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
    expect(out.text).toContain('Dear Hiring Team');
    // Research off → no brief on the result.
    expect(out.companyBrief).toBe('');
  });

  const COVER_META = {
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    mismatch: false,
    candidateName: 'X',
    jobTitle: 'Y',
    companyName: 'Z',
    targetLanguage: 'en',
    topRequirements: [],
  };

  const registerWithResearch = (research: ReturnType<typeof vi.fn>) => {
    const client = createMockClient({
      ai: {
        generatePipeline: vi.fn().mockResolvedValue({ jobId: 'gen-1' }),
        onStream: vi.fn((h: (chunk: unknown) => void) => {
          streamHandler = h;
          return () => {};
        }),
        researchCompany: research,
      },
      jobs: { get: vi.fn().mockResolvedValue(null), cancel: vi.fn() },
    });
    _registerClient(client);
    return client;
  };

  it('researches the company and folds the brief into the prompt when enabled', async () => {
    const research = vi
      .fn()
      .mockResolvedValue({ company: 'Acme', brief: 'Acme builds payment rails for SMBs.' });
    const client = registerWithResearch(research);

    const p = generateCoverLetter(
      'My resume',
      'Job ad at Acme',
      COVER_META,
      'recruiter',
      'llama3',
      vi.fn(),
      'en',
      undefined,
      undefined,
      { researchCompany: true }
    );
    await flushUntilStreaming();
    emit('Dear Hiring Team, great fit.');
    done();
    const out = await p;

    // The fetched brief is surfaced on the result so the caller can persist it.
    expect(out.companyBrief).toBe('Acme builds payment rails for SMBs.');
    expect(research).toHaveBeenCalledWith(expect.objectContaining({ jobAd: 'Job ad at Acme' }));
    expect(client.ai.generatePipeline).toHaveBeenCalledWith(
      expect.objectContaining({
        messages: expect.arrayContaining([
          expect.objectContaining({
            role: 'user',
            content: expect.stringContaining('Acme builds payment rails'),
          }),
        ]),
      })
    );
  });

  it('skips research entirely when the flag is off (no extra call)', async () => {
    const research = vi.fn().mockResolvedValue({ company: '', brief: '' });
    registerWithResearch(research);

    const p = generateCoverLetter(
      'My resume',
      'Job ad',
      COVER_META,
      'recruiter',
      'llama3',
      vi.fn()
    );
    await flushUntilStreaming();
    emit('Dear Hiring Team.');
    done();
    await p;

    expect(research).not.toHaveBeenCalled();
  });

  it('researchCompany degrades to an empty brief when the backend fails', async () => {
    registerWithResearch(vi.fn().mockRejectedValue(new Error('provider cannot search')));
    expect(await researchCompany('Job ad')).toBe('');
  });
});

describe('lookupSalaryRange (C2)', () => {
  const registerWithLookup = (lookupSalary: ReturnType<typeof vi.fn>) => {
    const client = createMockClient({
      ai: {
        generatePipeline: vi.fn().mockResolvedValue({ jobId: 'gen-1' }),
        onStream: vi.fn((h: (chunk: unknown) => void) => {
          streamHandler = h;
          return () => {};
        }),
        lookupSalary,
      },
      jobs: { get: vi.fn().mockResolvedValue(null), cancel: vi.fn() },
    });
    _registerClient(client);
    return client;
  };

  it('resolves the validated range and forwards role/company/location', async () => {
    const lookupSalary = vi.fn().mockResolvedValue({ min: 65000, max: 80000, currency: 'EUR' });
    const client = registerWithLookup(lookupSalary);

    const range = await lookupSalaryRange('Backend Engineer', 'Acme', 'Berlin, Germany');

    expect(range).toEqual({ min: 65000, max: 80000, currency: 'EUR' });
    expect(client.ai.lookupSalary).toHaveBeenCalledWith(
      expect.objectContaining({
        role: 'Backend Engineer',
        company: 'Acme',
        location: 'Berlin, Germany',
      })
    );
  });

  it('degrades to undefined when the backend finds nothing reliable', async () => {
    registerWithLookup(vi.fn().mockResolvedValue(null));
    const range = await lookupSalaryRange('Backend Engineer', 'Acme', 'Berlin');
    expect(range).toBeUndefined();
  });

  it('degrades to undefined (never throws) when the backend fails', async () => {
    registerWithLookup(vi.fn().mockRejectedValue(new Error('provider unavailable')));
    await expect(lookupSalaryRange('Backend Engineer', 'Acme', 'Berlin')).resolves.toBeUndefined();
  });
});

describe('researchAnswer', () => {
  const registerWithAnswerSearch = (answerSearch: ReturnType<typeof vi.fn>) => {
    const client = createMockClient({
      ai: {
        generatePipeline: vi.fn().mockResolvedValue({ jobId: 'gen-1' }),
        onStream: vi.fn((h: (chunk: unknown) => void) => {
          streamHandler = h;
          return () => {};
        }),
        researchAnswer: answerSearch,
      },
      jobs: { get: vi.fn().mockResolvedValue(null), cancel: vi.fn() },
    });
    _registerClient(client);
    return client;
  };

  it('resolves the notes and forwards the question/role/company', async () => {
    const answerSearch = vi.fn().mockResolvedValue('Acme raised a Series B in 2026.');
    const client = registerWithAnswerSearch(answerSearch);

    const notes = await researchAnswer('Why do you want to work here?', 'Backend Engineer', 'Acme');

    expect(notes).toBe('Acme raised a Series B in 2026.');
    expect(client.ai.researchAnswer).toHaveBeenCalledWith(
      expect.objectContaining({
        question: 'Why do you want to work here?',
        role: 'Backend Engineer',
        company: 'Acme',
      })
    );
  });

  it('degrades to an empty string when the backend finds nothing', async () => {
    registerWithAnswerSearch(vi.fn().mockResolvedValue(''));
    const notes = await researchAnswer('Why this role?', 'Engineer', 'Acme');
    expect(notes).toBe('');
  });

  it('degrades to an empty string (never throws) when the backend fails', async () => {
    registerWithAnswerSearch(vi.fn().mockRejectedValue(new Error('provider unavailable')));
    await expect(researchAnswer('Why this role?', 'Engineer', 'Acme')).resolves.toBe('');
  });
});

describe('generateApplicationAnswer', () => {
  it('grounds an answer prompt with the question + brief and returns clean text', async () => {
    const client = register();

    const p = generateApplicationAnswer({
      question: 'Why do you want to work here?',
      resume: 'My resume: led a payments migration.',
      jobAd: 'Backend role at Acme',
      meta: {
        resumeLanguage: 'en',
        jobAdLanguage: 'en',
        mismatch: false,
        candidateName: 'X',
        jobTitle: 'Backend Engineer',
        companyName: 'Acme',
        targetLanguage: 'en',
        topRequirements: [],
      },
      model: 'llama3',
      companyBrief: 'Acme builds payment rails.',
    });
    await flushUntilStreaming();
    emit('<think>plan</think>I led a payments migration, which maps to your rails work.');
    done();
    const answer = await p;

    expect(answer).toContain('payments migration');
    expect(answer).not.toContain('<think>');
    // The question and the (untrusted) brief both reach the user prompt.
    expect(client.ai.generatePipeline).toHaveBeenCalledWith(
      expect.objectContaining({
        messages: expect.arrayContaining([
          expect.objectContaining({
            role: 'user',
            content: expect.stringContaining('Why do you want to work here?'),
          }),
        ]),
      })
    );
    expect(client.ai.generatePipeline).toHaveBeenCalledWith(
      expect.objectContaining({
        messages: expect.arrayContaining([
          expect.objectContaining({
            role: 'user',
            content: expect.stringContaining('<company_research>'),
          }),
        ]),
      })
    );
  });

  it('folds opt-in web-search notes into a fenced <web_search_notes> block', async () => {
    const client = register();

    const p = generateApplicationAnswer({
      question: 'Why do you want to work here?',
      resume: 'My resume: led a payments migration.',
      jobAd: 'Backend role at Acme',
      meta: {
        resumeLanguage: 'en',
        jobAdLanguage: 'en',
        mismatch: false,
        candidateName: 'X',
        jobTitle: 'Backend Engineer',
        companyName: 'Acme',
        targetLanguage: 'en',
        topRequirements: [],
      },
      model: 'llama3',
      webSearchNotes: 'Acme recently announced a new product line.',
    });
    await flushUntilStreaming();
    emit('I led a payments migration relevant to your new product line.');
    done();
    await p;

    expect(client.ai.generatePipeline).toHaveBeenCalledWith(
      expect.objectContaining({
        messages: expect.arrayContaining([
          expect.objectContaining({
            role: 'user',
            content: expect.stringContaining('<web_search_notes>'),
          }),
        ]),
      })
    );
  });

  it('folds a market salary range into the prompt as a fenced <salary_context> block (C2)', async () => {
    const client = register();

    const p = generateApplicationAnswer({
      question: 'What are your salary expectations?',
      resume: 'My resume: led a payments migration.',
      jobAd: 'Backend role at Acme',
      meta: {
        resumeLanguage: 'en',
        jobAdLanguage: 'en',
        mismatch: false,
        candidateName: 'X',
        jobTitle: 'Backend Engineer',
        companyName: 'Acme',
        targetLanguage: 'en',
        topRequirements: [],
      },
      model: 'llama3',
      salaryRange: { min: 65000, max: 80000, currency: 'EUR' },
    });
    await flushUntilStreaming();
    emit('Open to discussing, given the market range. Number: 72500');
    done();
    await p;

    expect(client.ai.generatePipeline).toHaveBeenCalledWith(
      expect.objectContaining({
        messages: expect.arrayContaining([
          expect.objectContaining({
            role: 'user',
            content: expect.stringContaining('<salary_context>'),
          }),
        ]),
      })
    );
  });
});

describe('generateGitHubProjects', () => {
  const REPOS: GitHubRepo[] = [
    {
      name: 'merry-oasis',
      description: 'A local-first task planner.',
      htmlUrl: 'https://github.com/me/merry-oasis',
      language: 'TypeScript',
      topics: ['offline-first'],
      stars: 42,
    },
    {
      name: 'tiny-parser',
      description: 'Zero-dependency JSON parser.',
      htmlUrl: 'https://github.com/me/tiny-parser',
      language: 'Rust',
      topics: [],
      stars: 0,
    },
  ];

  it('parses AI entries and re-attaches each repo url as link (AI never writes the url)', async () => {
    const client = register();
    const p = generateGitHubProjects({ repos: REPOS, model: 'llama3' });
    await flushUntilStreaming();
    emit(
      [
        'NAME: Merry Oasis',
        'DESC: Built a local-first task planner • Implemented offline sync',
        '',
        'NAME: Tiny Parser',
        'DESC: Wrote a zero-dependency JSON parser in Rust',
      ].join('\n')
    );
    done();
    const out = await p;

    expect(out).toHaveLength(2);
    expect(out[0]).toEqual({
      name: 'Merry Oasis',
      description: 'Built a local-first task planner • Implemented offline sync',
      link: 'https://github.com/me/merry-oasis',
    });
    // Link is the repo's verbatim htmlUrl, in input order.
    expect(out[1]?.link).toBe('https://github.com/me/tiny-parser');
    // The repo metadata is fenced as untrusted in the prompt.
    expect(client.ai.generatePipeline).toHaveBeenCalledWith(
      expect.objectContaining({
        messages: expect.arrayContaining([
          expect.objectContaining({
            role: 'user',
            content: expect.stringContaining('<github_repos>'),
          }),
        ]),
      })
    );
  });

  it('falls back to the raw repo description + link for every repo when streaming throws', async () => {
    // generatePipeline rejects → streamGenerate throws → fallback path.
    const client = createMockClient({
      ai: { generatePipeline: vi.fn().mockRejectedValue(new Error('no provider configured')) },
      jobs: { get: vi.fn().mockResolvedValue(null), cancel: vi.fn() },
    });
    _registerClient(client);

    const out = await generateGitHubProjects({ repos: REPOS, model: 'llama3' });

    expect(out).toHaveLength(2);
    // Each entry falls back to the de-slugged name + raw description, with the link.
    expect(out[0]).toEqual({
      name: 'Merry Oasis',
      description: 'A local-first task planner.',
      link: 'https://github.com/me/merry-oasis',
    });
    expect(out[1]?.link).toBe('https://github.com/me/tiny-parser');
  });

  it('fills missing entries from the fallback when the model returns fewer than the repos', async () => {
    register();
    const p = generateGitHubProjects({ repos: REPOS, model: 'llama3' });
    await flushUntilStreaming();
    // Only the first repo gets an AI entry.
    emit('NAME: Merry Oasis\nDESC: Built a local-first task planner');
    done();
    const out = await p;

    expect(out).toHaveLength(2);
    expect(out[0]?.description).toBe('Built a local-first task planner');
    // Second repo falls back to its raw description, link still attached.
    expect(out[1]).toEqual({
      name: 'Tiny Parser',
      description: 'Zero-dependency JSON parser.',
      link: 'https://github.com/me/tiny-parser',
    });
  });

  it('survives a whole-response wrapped in one code fence (does not fall back)', async () => {
    // A local model wraps its entire answer in a single ``` block. extractPlainText
    // would delete it, dropping every AI entry to the raw-description fallback; the
    // wrapper parses the raw stream so the AI entries survive.
    register();
    const p = generateGitHubProjects({ repos: REPOS, model: 'llama3' });
    await flushUntilStreaming();
    emit(
      [
        '```',
        'NAME: Merry Oasis',
        'DESC: Built a local-first task planner',
        '',
        'NAME: Tiny Parser',
        'DESC: Wrote a zero-dependency JSON parser',
        '```',
      ].join('\n')
    );
    done();
    const out = await p;

    // AI output survived — NOT the raw GitHub descriptions.
    expect(out[0]?.description).toBe('Built a local-first task planner');
    expect(out[1]?.description).toBe('Wrote a zero-dependency JSON parser');
    expect(out[0]?.link).toBe('https://github.com/me/merry-oasis');
  });

  it('matches each AI entry to its repo by name so links never cross when blocks reorder', async () => {
    register();
    const p = generateGitHubProjects({ repos: REPOS, model: 'llama3' });
    await flushUntilStreaming();
    // Model emits the blocks in REVERSE order (tiny-parser first).
    emit(
      [
        'NAME: Tiny Parser',
        'DESC: Wrote a zero-dependency JSON parser',
        '',
        'NAME: Merry Oasis',
        'DESC: Built a local-first task planner',
      ].join('\n')
    );
    done();
    const out = await p;

    // Output stays in INPUT order, and each description lands on the repo whose
    // link it belongs to — positional pairing would have crossed them.
    expect(out[0]).toEqual({
      name: 'Merry Oasis',
      description: 'Built a local-first task planner',
      link: 'https://github.com/me/merry-oasis',
    });
    expect(out[1]).toEqual({
      name: 'Tiny Parser',
      description: 'Wrote a zero-dependency JSON parser',
      link: 'https://github.com/me/tiny-parser',
    });
  });

  it('returns [] without calling the provider for an empty repo list', async () => {
    const client = register();
    expect(await generateGitHubProjects({ repos: [], model: 'llama3' })).toEqual([]);
    expect(client.ai.generatePipeline).not.toHaveBeenCalled();
  });

  // ── item 5: fallbackProject with absent description ───────────────────────

  it('fallback uses de-slugged repo name when description is absent', async () => {
    // A repo with NO description field — not even null — must fall back to the
    // de-slugged name, not '' or undefined. "my-cool-app" → "My Cool App".
    const repoNoDesc: GitHubRepo = {
      name: 'my-cool-app',
      htmlUrl: 'https://github.com/u/my-cool-app',
      language: 'TypeScript',
      topics: [],
      stars: 0,
      // description intentionally absent
    };
    const client = createMockClient({
      ai: { generatePipeline: vi.fn().mockRejectedValue(new Error('no provider')) },
      jobs: { get: vi.fn().mockResolvedValue(null), cancel: vi.fn() },
    });
    _registerClient(client);

    const out = await generateGitHubProjects({ repos: [repoNoDesc], model: 'llama3' });

    expect(out).toHaveLength(1);
    // Description falls back to de-slugged name (not '' or undefined).
    expect(out[0]?.description).toBe('My Cool App');
    // Link is always the repo's own htmlUrl.
    expect(out[0]?.link).toBe('https://github.com/u/my-cool-app');
  });

  // ── item 6: name-match dedup guard — two repos normalizing to same key ────

  it('dedup guard: two repos with same normalized key each keep their own link', async () => {
    // "my-app" and "My App" both normalize to "myapp". The model returns ONE
    // matching block ("My App"). The second repo must NOT claim the first entry
    // twice — it must fall back to its own link via the positional or fallback path.
    const repoA: GitHubRepo = {
      name: 'my-app',
      description: 'Slug version',
      htmlUrl: 'https://github.com/u/my-app',
      language: 'TypeScript',
      topics: [],
      stars: 5,
    };
    const repoB: GitHubRepo = {
      name: 'My App',
      description: 'Space version',
      htmlUrl: 'https://github.com/u/My-App',
      language: 'Rust',
      topics: [],
      stars: 2,
    };

    register();
    const p = generateGitHubProjects({ repos: [repoA, repoB], model: 'llama3' });
    await flushUntilStreaming();
    // Model emits only ONE block that matches the normalized key "myapp".
    emit('NAME: My App\nDESC: AI-generated bullet for the app');
    done();
    const out = await p;

    expect(out).toHaveLength(2);
    // First repo claimed the name-match entry — gets the AI description.
    expect(out[0]?.link).toBe('https://github.com/u/my-app');
    // Second repo must NOT re-claim the same entry — gets its own link via fallback.
    expect(out[1]?.link).toBe('https://github.com/u/My-App');
    // Links must not cross.
    expect(out[0]?.link).not.toBe(out[1]?.link);
  });

  // ── item 7: pre-aborted AbortSignal returns fallback array, no throw ──────

  it('pre-aborted signal returns raw-description fallback array without throwing', async () => {
    // An already-cancelled AbortSignal: generateGitHubProjects must return one
    // fallback entry per repo (with link), not throw.
    register();

    const controller = new AbortController();
    controller.abort();

    const out = await generateGitHubProjects({
      repos: REPOS,
      model: 'llama3',
      signal: controller.signal,
    });

    // Must return one entry per repo.
    expect(out).toHaveLength(2);
    // Each entry has the repo's own htmlUrl as link.
    expect(out[0]?.link).toBe('https://github.com/me/merry-oasis');
    expect(out[1]?.link).toBe('https://github.com/me/tiny-parser');
    // Both entries have non-empty descriptions (fallback to raw description).
    expect(out[0]?.description).toBeTruthy();
    expect(out[1]?.description).toBeTruthy();
  });
});

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
    const p = extractMetadata('resume', 'job ad', 'llama3');
    await flushUntilStreaming();
    emit('{}');
    done();
    await p;

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
    const p = extractMetadata('resume', 'job ad', 'gpt-4o');
    await flushUntilStreaming();
    emit('{}');
    done();
    await p;

    const call = (client.ai.generatePipeline as ReturnType<typeof vi.fn>).mock.calls[0];
    expect(call).toBeDefined();
    const arg = call?.[0] as { provider?: string; contextWindow?: number };
    // Routing no longer crosses the wire, and the cloud path sends no local ctx.
    expect(arg.provider).toBeUndefined();
    expect(arg.contextWindow).toBeUndefined();
  });
});

describe('per-step temperature override', () => {
  // The resolved temperature is forwarded verbatim to generatePipeline, so we
  // assert the wiring end-to-end through the public generation functions.
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

  const tempOf = (client: ReturnType<typeof register>) => {
    const call = (client.ai.generatePipeline as ReturnType<typeof vi.fn>).mock.calls[0];
    return (call?.[0] as { temperature: number }).temperature;
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

  it('applies the per-step override to its own step only (cover set, resume default)', async () => {
    setOllama({ cover: 0.85 });
    const client = register();
    const p = generateCoverLetter('My resume', 'Job ad', META, 'recruiter', 'llama3', vi.fn());
    await flushUntilStreaming();
    emit('Dear Hiring Team.');
    done();
    await p;
    // cover override resolves; the small tier's 0.58 default is overridden.
    expect(tempOf(client)).toBeCloseTo(0.85);
  });

  it('falls back to the step default when that step is undefined', async () => {
    // Only `cover` is set — résumé generation must still use its 0.3 default.
    setOllama({ cover: 0.85 });
    const client = register();
    const p = generateResume('My resume', 'Job ad', META, 'ats', 'llama3', vi.fn());
    await flushUntilStreaming();
    emit('RESUME CONTENT');
    done();
    await p;
    expect(tempOf(client)).toBeCloseTo(0.3);
  });

  it('ignores the override for non-ollama providers (always the step default)', async () => {
    // Cloud provider active, but ollama still carries a cover override: must be ignored.
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
    const p = generateCoverLetter('My resume', 'Job ad', META, 'recruiter', 'gpt-4o', vi.fn());
    await flushUntilStreaming();
    emit('Dear Hiring Team.');
    done();
    await p;
    // large tier default for cloud = 0.8; the ollama override does not apply.
    expect(tempOf(client)).toBeCloseTo(0.8);
  });
});

describe('prose detector-resistance sampling params', () => {
  // RAID (ACL 2024): random sampling + repetition/frequency penalties drop
  // AI-detector accuracy. Applied only to prose surfaces — resume/analysis stay
  // LEXICAL (no new params) to protect exact ATS keyword repetition.
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

  const samplingOf = (client: ReturnType<typeof register>) => {
    const call = (client.ai.generatePipeline as ReturnType<typeof vi.fn>).mock.calls[0];
    const arg = call?.[0] as {
      temperature?: number;
      topP?: number;
      frequencyPenalty?: number;
      presencePenalty?: number;
      repeatPenalty?: number;
    };
    const { temperature, topP, frequencyPenalty, presencePenalty, repeatPenalty } = arg;
    return { temperature, topP, frequencyPenalty, presencePenalty, repeatPenalty };
  };

  it('cover letter (small-tier local model) tightens topP to limit drift', async () => {
    // 'llama3' has no size suffix → classified small tier (see model-size.ts);
    // small local models compound drift when the full 0.95 topP stacks with
    // repeatPenalty, so the small tier tightens topP to 0.9.
    const client = register();
    const p = generateCoverLetter('My resume', 'Job ad', META, 'recruiter', 'llama3', vi.fn());
    await flushUntilStreaming();
    emit('Dear Hiring Team.');
    done();
    await p;
    expect(samplingOf(client)).toEqual({
      temperature: 0.58,
      topP: 0.9,
      frequencyPenalty: 0.3,
      presencePenalty: 0.2,
      repeatPenalty: 1.15,
    });
  });

  it('cover letter (large-tier cloud model) keeps the shared topP default', async () => {
    setActive('openai', 'gpt-4o');
    usePreferencesStore.setState({
      aiProviderConfig: { activeProvider: 'openai', providers: { openai: { model: 'gpt-4o' } } },
    });
    const client = register();
    const p = generateCoverLetter('My resume', 'Job ad', META, 'recruiter', 'gpt-4o', vi.fn());
    await flushUntilStreaming();
    emit('Dear Hiring Team.');
    done();
    await p;
    expect(samplingOf(client)).toEqual({
      temperature: 0.8,
      topP: 0.95,
      frequencyPenalty: 0.3,
      presencePenalty: 0.2,
      repeatPenalty: 1.15,
    });
  });

  it('application answer generation drops presencePenalty and lowers temperature (no-fabrication surface)', async () => {
    // Résumé-grounded: presencePenalty pushes toward new topics (fabrication
    // risk) so it's dropped; temperature stays lower than the freer prose
    // surfaces to limit factual drift, while topP/frequencyPenalty/
    // repeatPenalty still resist AI-detector fingerprinting.
    const client = register();
    const p = generateApplicationAnswer({
      question: 'Why do you want to work here?',
      resume: 'My resume',
      jobAd: 'Backend role at Acme',
      meta: META,
      model: 'llama3',
    });
    await flushUntilStreaming();
    emit('Because I love building things.');
    done();
    await p;
    expect(samplingOf(client)).toEqual({
      temperature: 0.5,
      topP: 0.95,
      frequencyPenalty: 0.3,
      presencePenalty: undefined,
      repeatPenalty: 1.15,
    });
  });

  it('resume generation carries no sampling params (LEXICAL — protects ATS keyword repetition)', async () => {
    const client = register();
    const p = generateResume('My resume', 'Job ad', META, 'ats', 'llama3', vi.fn());
    await flushUntilStreaming();
    emit('RESUME CONTENT');
    done();
    await p;
    const sampling = samplingOf(client);
    expect(sampling.topP).toBeUndefined();
    expect(sampling.frequencyPenalty).toBeUndefined();
    expect(sampling.presencePenalty).toBeUndefined();
    expect(sampling.repeatPenalty).toBeUndefined();
  });

  it('metadata extraction (analysis) carries no sampling params', async () => {
    const client = register();
    const p = extractMetadata('My resume', 'Job ad', 'llama3');
    await flushUntilStreaming();
    emit('{}');
    done();
    await p;
    const sampling = samplingOf(client);
    expect(sampling.topP).toBeUndefined();
    expect(sampling.frequencyPenalty).toBeUndefined();
    expect(sampling.presencePenalty).toBeUndefined();
    expect(sampling.repeatPenalty).toBeUndefined();
  });
});

describe('output tone wiring (Settings → Output Tone)', () => {
  // The system prompt is always messages[0] (see streamGenerate in generation.ts).
  const systemOf = (client: ReturnType<typeof register>) => {
    const call = (client.ai.generatePipeline as ReturnType<typeof vi.fn>).mock.calls[0];
    const messages = (call?.[0] as { messages: { role: string; content: string }[] }).messages;
    return messages[0]?.content ?? '';
  };

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

  it('threads the store outputTone into the resume system prompt', async () => {
    usePreferencesStore.setState({ outputTone: 'casual' });
    const client = register();
    const p = generateResume('My resume', 'Job ad', META, 'ats', 'llama3', vi.fn());
    await flushUntilStreaming();
    emit('RESUME CONTENT');
    done();
    await p;
    expect(systemOf(client)).toMatch(/TONE: conversational and casual/);
  });

  it('threads the store outputTone into the cover-letter system prompt', async () => {
    usePreferencesStore.setState({ outputTone: 'formal' });
    const client = register();
    const p = generateCoverLetter('My resume', 'Job ad', META, 'recruiter', 'llama3', vi.fn());
    await flushUntilStreaming();
    emit('Dear Hiring Team.');
    done();
    await p;
    expect(systemOf(client)).toMatch(/TONE: formal and precise/);
  });

  it('threads the store outputTone into the application-answer system prompt', async () => {
    usePreferencesStore.setState({ outputTone: 'creative' });
    const client = register();
    const p = generateApplicationAnswer({
      question: 'Why do you want to work here?',
      resume: 'My resume',
      jobAd: 'Backend role at Acme',
      meta: META,
      model: 'llama3',
    });
    await flushUntilStreaming();
    emit('Because I love building things.');
    done();
    await p;
    expect(systemOf(client)).toMatch(/TONE: a more narrative, distinctive voice/);
  });

  it('resolves to the professional tone directive by default (outputTone: professional)', async () => {
    const client = register();
    const p = generateResume('My resume', 'Job ad', META, 'ats', 'llama3', vi.fn());
    await flushUntilStreaming();
    emit('RESUME CONTENT');
    done();
    await p;
    expect(systemOf(client)).toMatch(/TONE: polished, warm, and professional/);
  });
});
