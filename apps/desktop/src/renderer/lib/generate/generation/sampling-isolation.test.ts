import { describe, expect, it, type vi } from 'vitest';

import type { GitHubRepo } from '@ajh/shared';

import { usePreferencesStore } from '@/store/preferences-store';

import {
  generateApplicationAnswer,
  generateGitHubProjects,
  generateInterviewQuestions,
  generateJobAdSummary,
  generateLikelyInterviewQuestions,
  generateStarFeedback,
} from './generation';
import {
  done,
  emit,
  flushUntilStreaming,
  installGenerationHooks,
  register,
  setActive,
  streamThrough,
} from './test-support';

installGenerationHooks();

describe('per-step override isolation (regression: one `answers` slider used to span 4 unrelated surfaces)', () => {
  // Before this fix, `answers` backed application answers (prose_grounded),
  // job-ad summary + GitHub-project import (deterministic), AND every
  // interview-prep surface (prose) — one slider silently retuning surfaces a
  // user never meant to touch. Each test below sets a SIBLING step's
  // override and asserts it does NOT leak into the surface under test —
  // the assertion that would have caught the original bug.
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

  const REPOS: GitHubRepo[] = [
    {
      name: 'merry-oasis',
      description: 'A local-first task planner.',
      htmlUrl: 'https://github.com/me/merry-oasis',
      language: 'TypeScript',
      topics: [],
      stars: 1,
    },
  ];

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

  it('job-ad summary ignores an `answers` override (was the bug — now keyed off `analysis`)', async () => {
    setOllama({ answers: 0.99 });
    const client = register();
    await streamThrough(
      generateJobAdSummary({ jobAd: 'Backend role at Acme', model: 'llama3' }),
      'Key notes.'
    );
    expect(argOf(client).temperature).toBeUndefined();
  });

  it('job-ad summary honors its own `analysis` override', async () => {
    setOllama({ analysis: 0.15 });
    const client = register();
    await streamThrough(
      generateJobAdSummary({ jobAd: 'Backend role at Acme', model: 'llama3' }),
      'Key notes.'
    );
    expect(argOf(client).temperature).toBeCloseTo(0.15);
    expect(argOf(client).intent).toBe('deterministic');
  });

  it('GitHub project import ignores an `answers` override (was the bug — now keyed off `resume`)', async () => {
    setOllama({ answers: 0.99 });
    const client = register();
    await streamThrough(
      generateGitHubProjects({ repos: REPOS, model: 'llama3' }),
      'NAME: Merry Oasis\nDESC: Built a local-first task planner.'
    );
    expect(argOf(client).temperature).toBeUndefined();
  });

  it('GitHub project import honors its own `resume` override', async () => {
    setOllama({ resume: 0.2 });
    const client = register();
    await streamThrough(
      generateGitHubProjects({ repos: REPOS, model: 'llama3' }),
      'NAME: Merry Oasis\nDESC: Built a local-first task planner.'
    );
    expect(argOf(client).temperature).toBeCloseTo(0.2);
    expect(argOf(client).intent).toBe('deterministic');
  });

  it('interview questions ignore an `answers` override (was the bug — now keyed off `questions`)', async () => {
    setOllama({ answers: 0.99 });
    const client = register();
    await streamThrough(
      generateInterviewQuestions({
        resume: 'My resume',
        jobAd: 'Job ad',
        meta: META,
        model: 'llama3',
      }),
      'Q: What draws you to this role?'
    );
    expect(argOf(client).temperature).toBeUndefined();
  });

  it('interview questions, likely questions, and STAR feedback all honor a shared `questions` override', async () => {
    setOllama({ questions: 0.55 });

    let client = register();
    let p = generateInterviewQuestions({
      resume: 'My resume',
      jobAd: 'Job ad',
      meta: META,
      model: 'llama3',
    });
    await flushUntilStreaming();
    emit('Q: What draws you to this role?');
    done();
    await p;
    expect(argOf(client).temperature).toBeCloseTo(0.55);
    expect(argOf(client).intent).toBe('prose');

    client = register();
    p = generateLikelyInterviewQuestions({
      resume: 'My resume',
      jobAd: 'Job ad',
      meta: META,
      model: 'llama3',
    });
    await flushUntilStreaming();
    emit('Q: Tell me about yourself.');
    done();
    await p;
    expect(argOf(client).temperature).toBeCloseTo(0.55);
    expect(argOf(client).intent).toBe('prose');

    client = register();
    p = generateStarFeedback({
      question: 'Tell me about a time you led a project.',
      answer: 'I led the migration.',
      resume: 'My resume',
      jobAd: 'Job ad',
      meta: META,
      model: 'llama3',
    });
    await flushUntilStreaming();
    emit('Strong structure, add a measurable result.');
    done();
    await p;
    expect(argOf(client).temperature).toBeCloseTo(0.55);
    expect(argOf(client).intent).toBe('prose');
  });

  it('application answers ignore a `questions` override — `answers` stays its own surface', async () => {
    setOllama({ questions: 0.99 });
    const client = register();
    await streamThrough(
      generateApplicationAnswer({
        question: 'Why do you want to work here?',
        resume: 'My resume',
        jobAd: 'Backend role at Acme',
        meta: META,
        model: 'llama3',
      }),
      'Because I love building things.'
    );
    expect(argOf(client).temperature).toBeUndefined();
    expect(argOf(client).intent).toBe('prose_grounded');
  });
});
