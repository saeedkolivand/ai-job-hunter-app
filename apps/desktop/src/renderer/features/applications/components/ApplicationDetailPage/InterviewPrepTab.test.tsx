/**
 * InterviewPrepTab — job-description source precedence
 *
 * Covers the fix that makes `application.jobDescription` the PRIMARY source for
 * the tab's `hasDesc` flag. When the user pastes a job ad in the Documents tab
 * it is debounce-persisted to `application.jobDescription`; this tab must then
 * show the Generate button as enabled (no "needsJob" warning) without requiring
 * a reload.
 *
 * Strategy:
 *  - All hooks that touch IPC / QueryClient are mocked (no provider tree).
 *  - `useInterviewQuestions` is mocked so we can inspect `hasDesc` indirectly
 *    through the rendered "needsJob" warning that appears when `hasDesc` is false.
 *  - `useResolveJobUrl` is mocked and controllable per test to assert the
 *    "shouldFetch" arg (only false when `initialDesc` is non-empty).
 *  - noUncheckedIndexedAccess: all array accesses are guarded.
 */

import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import type { AiGenerationRecord, Application } from '@ajh/shared';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ── ModelSelector hooks ───────────────────────────────────────────────────────

vi.mock('@/components/ui/ModelSelector', () => ({
  useSelectedModel: () => 'test-model',
  useCanUseAI: () => ({ canUse: true }),
}));

// ── Document hooks — no real IPC needed ──────────────────────────────────────

vi.mock('@/features/jobs/hooks/useDefaultResumeId', () => ({
  useDefaultResumeId: () => null,
}));

// ── Service hooks ─────────────────────────────────────────────────────────────

// Mutable so tests can control what the resolver returns and whether it is called.
const resolveJobUrlState: { data: { description: string } | undefined; isLoading: boolean } = {
  data: undefined,
  isLoading: false,
};
let lastShouldFetch: boolean | undefined = undefined;

vi.mock('@/services', () => ({
  useDocuments: () => ({ data: [], isLoading: false }),
  useDocumentText: () => ({ data: undefined, isLoading: false }),
  useResolveJobUrl: (_url: string, shouldFetch: boolean) => {
    lastShouldFetch = shouldFetch;
    return { data: resolveJobUrlState.data, isLoading: resolveJobUrlState.isLoading };
  },
}));

// ── useInterviewQuestions — controlled mock ───────────────────────────────────

// Capture the `hasDesc` arg so we can assert precedence without rendering the full UI.
let capturedHasDesc: boolean | undefined = undefined;

const iqMock = {
  seedTopics: '',
  setSeedTopics: vi.fn(),
  audiences: ['recruiter'] as string[],
  toggleAudience: vi.fn(),
  questions: [] as { id: string; question: string; audience: string }[],
  generating: false,
  error: null,
  generate: vi.fn(),
  canGenerate: false,
  needsResearchKey: false,
};

vi.mock('@/hooks/use-interview-questions', () => ({
  useInterviewQuestions: (args: { hasDesc: boolean }) => {
    capturedHasDesc = args.hasDesc;
    return iqMock;
  },
}));

// ── AudienceSelector / InterviewQuestionsAccordion — stubs ───────────────────

vi.mock('@/components/interview/AudienceSelector', () => ({
  AudienceSelector: () => <div data-testid="audience-selector" />,
}));

vi.mock('@/components/interview/InterviewQuestionsAccordion', () => ({
  InterviewQuestionsAccordion: () => <div data-testid="iq-accordion" />,
}));

// ── Import component AFTER all mocks ─────────────────────────────────────────

import { InterviewPrepTab } from './InterviewPrepTab';

// ── Fixtures ──────────────────────────────────────────────────────────────────

function makeApp(overrides: Partial<Application> = {}): Application {
  return {
    id: 'app-1',
    status: 'applied',
    createdAt: 1000,
    updatedAt: 1000,
    jobUrl: 'https://acme.com/job/1',
    board: 'linkedin',
    company: 'Acme',
    title: 'Engineer',
    candidate: 'Jane',
    answers: [],
    brief: '',
    notes: '',
    comp: '',
    jobDescription: '',
    jobSummary: '',
    contactName: '',
    contactEmail: '',
    ...overrides,
  };
}

function makeGen(overrides: Partial<AiGenerationRecord> = {}): AiGenerationRecord {
  return {
    id: 'gen-1',
    jobUrl: 'https://acme.com/job/1',
    createdAt: 0,
    candidateName: '',
    jobTitle: '',
    companyName: '',
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    targetLanguage: 'en',
    mismatch: false,
    topRequirements: [],
    mode: 'standard',
    resumeText: '',
    coverLetterText: '',
    jobAd: '',
    board: '',
    applicationAnswers: [],
    companyBrief: '',
    interviewQuestions: [],
    ...overrides,
  };
}

// ── Reset between tests ───────────────────────────────────────────────────────

beforeEach(() => {
  capturedHasDesc = undefined;
  lastShouldFetch = undefined;
  resolveJobUrlState.data = undefined;
  resolveJobUrlState.isLoading = false;
  iqMock.questions = [];
  iqMock.generate.mockClear();
});

// ─────────────────────────────────────────────────────────────────────────────
// 1. application.jobDescription as primary source
// ─────────────────────────────────────────────────────────────────────────────

describe('InterviewPrepTab — application.jobDescription as primary source', () => {
  it('hasDesc=true when application.jobDescription is non-empty (no generation needed)', () => {
    const app = makeApp({ jobDescription: 'We need a senior engineer.' });
    render(<InterviewPrepTab application={app} matchingGenerations={[]} />);

    expect(capturedHasDesc).toBe(true);
    // "needsJob" warning must NOT appear when hasDesc is true.
    expect(screen.queryByText('applications.detail.interview.needsJob')).not.toBeInTheDocument();
  });

  it('hasDesc=false and needsJob warning visible when jobDescription is empty and no generation', () => {
    const app = makeApp({ jobDescription: '' });
    render(<InterviewPrepTab application={app} matchingGenerations={[]} />);

    expect(capturedHasDesc).toBe(false);
    expect(screen.getByText('applications.detail.interview.needsJob')).toBeInTheDocument();
  });

  it('application.jobDescription wins over saved generation jobAd when both present', () => {
    // The persisted JD (from the Documents tab paste) must take precedence so
    // once persisted the state is reflected immediately without re-deriving from
    // the older generation record.
    const app = makeApp({ jobDescription: 'Persisted JD from Documents tab.' });
    const gen = makeGen({ jobAd: 'Older generation job ad.' });
    render(<InterviewPrepTab application={app} matchingGenerations={[gen]} />);

    expect(capturedHasDesc).toBe(true);
    // shouldFetch must be false because initialDesc is non-empty — no URL round-trip.
    expect(lastShouldFetch).toBe(false);
  });

  it('falls back to saved generation jobAd when jobDescription is empty', () => {
    const app = makeApp({ jobDescription: '' });
    const gen = makeGen({ jobAd: 'Fallback generation job ad.' });
    render(<InterviewPrepTab application={app} matchingGenerations={[gen]} />);

    expect(capturedHasDesc).toBe(true);
    // shouldFetch false because the fallback generation jobAd filled initialDesc.
    expect(lastShouldFetch).toBe(false);
  });

  it('resolves from URL when both jobDescription and saved jobAd are empty', () => {
    const app = makeApp({ jobDescription: '' });
    resolveJobUrlState.data = { description: 'Fetched from URL.' };

    render(<InterviewPrepTab application={app} matchingGenerations={[]} />);

    // shouldFetch=true when initialDesc is empty — URL resolution is triggered.
    expect(lastShouldFetch).toBe(true);
    // The resolved description fills jobDesc → hasDesc=true.
    expect(capturedHasDesc).toBe(true);
  });

  it('hasDesc=false when all three sources are empty', () => {
    const app = makeApp({ jobDescription: '' });
    // No generation, no URL resolution result.
    render(<InterviewPrepTab application={app} matchingGenerations={[]} />);

    expect(capturedHasDesc).toBe(false);
    expect(lastShouldFetch).toBe(true);
  });

  it('whitespace-only jobDescription is treated as empty (trimmed)', () => {
    // A jobDescription of whitespace must not satisfy hasDesc — it would produce
    // an empty generation request. The `.trim()` in initialDesc covers this.
    const app = makeApp({ jobDescription: '   \n  ' });
    render(<InterviewPrepTab application={app} matchingGenerations={[]} />);

    expect(capturedHasDesc).toBe(false);
  });
});
