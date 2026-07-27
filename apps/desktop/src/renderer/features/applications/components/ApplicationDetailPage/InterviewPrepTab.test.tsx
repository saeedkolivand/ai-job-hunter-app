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
import { fireEvent, render, screen } from '@testing-library/react';

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
  language: 'en',
  detectedLanguage: 'en',
  setLanguage: vi.fn(),
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

// ── useInterviewPractice — controlled mock (session-only, no aggregate) ──────

const practiceMock = {
  questions: [] as { id: string; question: string; type: string }[],
  generating: false,
  error: null as string | null,
  generate: vi.fn(),
  canGenerate: true,
  feedback: {} as Record<string, unknown>,
  getFeedback: vi.fn(),
};

vi.mock('@/hooks/use-interview-practice', () => ({
  useInterviewPractice: () => practiceMock,
}));

// ── AudienceSelector / InterviewQuestionsAccordion / InterviewPracticePanel — stubs ──

vi.mock('@/components/interview/AudienceSelector', () => ({
  AudienceSelector: () => <div data-testid="audience-selector" />,
}));

vi.mock('@/components/interview/InterviewQuestionsAccordion', () => ({
  InterviewQuestionsAccordion: () => <div data-testid="iq-accordion" />,
}));

vi.mock('@/components/interview/InterviewPracticePanel', () => ({
  InterviewPracticePanel: () => <div data-testid="practice-panel" />,
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
  iqMock.language = 'en';
  iqMock.detectedLanguage = 'en';
  iqMock.setLanguage.mockClear();
  iqMock.generate.mockClear();
  practiceMock.generate.mockClear();
  practiceMock.getFeedback.mockClear();
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

// ─────────────────────────────────────────────────────────────────────────────
// 2. Mode toggle — "Questions to ask" vs "Practice answers"
// ─────────────────────────────────────────────────────────────────────────────

describe('InterviewPrepTab — mode toggle', () => {
  it('defaults to "ask" mode: the accordion body renders, the practice panel does not', () => {
    render(<InterviewPrepTab application={makeApp()} matchingGenerations={[]} />);

    expect(screen.getByTestId('audience-selector')).toBeInTheDocument();
    expect(screen.queryByTestId('practice-panel')).not.toBeInTheDocument();
  });

  it('switches to the practice panel when "Practice answers" is selected, hiding the ask-mode toolbar', () => {
    render(<InterviewPrepTab application={makeApp()} matchingGenerations={[]} />);

    fireEvent.click(screen.getByText('applications.detail.interview.toggle.practice'));

    expect(screen.getByTestId('practice-panel')).toBeInTheDocument();
    expect(screen.queryByTestId('audience-selector')).not.toBeInTheDocument();
  });

  it('switches back to "ask" mode', () => {
    render(<InterviewPrepTab application={makeApp()} matchingGenerations={[]} />);

    fireEvent.click(screen.getByText('applications.detail.interview.toggle.practice'));
    fireEvent.click(screen.getByText('applications.detail.interview.toggle.ask'));

    expect(screen.getByTestId('audience-selector')).toBeInTheDocument();
    expect(screen.queryByTestId('practice-panel')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 3. Output-language selector (ask-mode toolbar)
// ─────────────────────────────────────────────────────────────────────────────

describe('InterviewPrepTab — output language selector', () => {
  /** The dropdown trigger, named by the sr-only <label htmlFor="iq-language">. */
  const trigger = () => screen.getByLabelText('applications.detail.interview.languageLabel');

  it('shows the hook language as the selected option, labelled by its endonym', () => {
    iqMock.language = 'de';
    render(<InterviewPrepTab application={makeApp()} matchingGenerations={[]} />);

    expect(trigger()).toHaveTextContent('Deutsch');
  });

  it('forwards a picked language to the hook as a locale CODE', () => {
    render(<InterviewPrepTab application={makeApp()} matchingGenerations={[]} />);

    fireEvent.click(trigger());
    fireEvent.click(screen.getByText('Français'));

    expect(iqMock.setLanguage).toHaveBeenCalledWith('fr');
  });

  it('names a detected language the picker does not stock, instead of showing English', () => {
    // Dutch is not in OUTPUT_LANGUAGES, but it IS what generation will use.
    iqMock.language = 'nl';
    iqMock.detectedLanguage = 'nl';
    render(<InterviewPrepTab application={makeApp()} matchingGenerations={[]} />);

    expect(trigger()).toHaveTextContent('Dutch');
    expect(trigger()).not.toHaveTextContent('English');
  });

  it('offers the ad-language option as "match the job ad" when nothing was detected', () => {
    iqMock.language = '';
    iqMock.detectedLanguage = '';
    render(<InterviewPrepTab application={makeApp()} matchingGenerations={[]} />);

    expect(trigger()).toHaveTextContent('applications.detail.interview.languageAuto');
  });

  it('keeps the ad-language option reachable after an explicit pick', () => {
    // The user picked Spanish, but the ad is Dutch — the Dutch option must still
    // be listed so the choice can be undone.
    iqMock.language = 'es';
    iqMock.detectedLanguage = 'nl';
    render(<InterviewPrepTab application={makeApp()} matchingGenerations={[]} />);

    expect(trigger()).toHaveTextContent('Español');
    fireEvent.click(trigger());
    fireEvent.click(screen.getByText('Dutch'));

    expect(iqMock.setLanguage).toHaveBeenCalledWith('nl');
  });

  it('does not duplicate a detected language the picker already stocks', () => {
    iqMock.language = 'de';
    iqMock.detectedLanguage = 'de';
    render(<InterviewPrepTab application={makeApp()} matchingGenerations={[]} />);

    fireEvent.click(trigger());
    // Exactly one "Deutsch" OPTION — no synthetic duplicate alongside the real
    // one. (Scoped to role=option: the trigger also renders the selected label.)
    expect(screen.getAllByRole('option', { name: 'Deutsch' })).toHaveLength(1);
  });

  it('is part of the ask-mode toolbar only (hidden in practice mode)', () => {
    render(<InterviewPrepTab application={makeApp()} matchingGenerations={[]} />);
    expect(trigger()).toBeInTheDocument();

    fireEvent.click(screen.getByText('applications.detail.interview.toggle.practice'));

    expect(
      screen.queryByLabelText('applications.detail.interview.languageLabel')
    ).not.toBeInTheDocument();
  });
});
