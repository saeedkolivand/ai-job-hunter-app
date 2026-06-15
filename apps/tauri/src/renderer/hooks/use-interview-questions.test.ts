import { createElement, type ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';

import { generateInterviewQuestions } from '@/lib/generate';

import { useInterviewQuestions } from './use-interview-questions';

// Capture the persisted record so we can assert the generated questions + audiences.
const save = vi.fn().mockResolvedValue({ id: 'gen-1', success: true });
vi.mock('@/providers/AppClientProvider', () => ({
  useAppClient: () => ({ aiGenerations: { save } }),
}));

// Keys object only — keep the hook isolated from the real query-client graph.
vi.mock('@/services/query-client', () => ({
  keys: { aiGenerations: { all: ['aiGenerations'] }, autopilot: { all: ['autopilot'] } },
}));

// Stub the generation pipeline. `parseInterviewQuestions` returns a fixed set so we
// can assert it lands on `questions`; `generateInterviewQuestions` is the seam we
// assert the selected audiences flow into.
vi.mock('@/lib/generate', () => ({
  extractMetadata: vi.fn().mockResolvedValue({
    candidateName: 'Ada',
    jobTitle: 'Engineer',
    companyName: 'Acme',
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    mismatch: false,
    targetLanguage: 'en',
    topRequirements: [],
  }),
  researchCompany: vi.fn().mockResolvedValue('BRIEF'),
  generateInterviewQuestions: vi.fn().mockResolvedValue('RAW'),
  parseInterviewQuestions: vi
    .fn()
    .mockReturnValue([{ id: 'iq-1', question: 'Q1', why: 'because', audience: 'recruiter' }]),
}));

// Active provider + Ollama web-search key — controllable so we can exercise the
// needsResearchKey hint (Ollama-family provider without the key).
let mockProvider = 'openai';
let mockHasOllamaKey = true;
vi.mock('@/components/ui/ModelSelector', () => ({ useSelectedProvider: () => mockProvider }));
vi.mock('@/services', () => ({ useHasProviderKey: () => ({ data: { has: mockHasOllamaKey } }) }));

const params = {
  resume: 'my resume',
  jobDesc: 'JD',
  model: 'llama',
  canUse: true,
  hasDesc: true,
  jobUrl: 'https://acme.com/job/42',
  board: 'linkedin',
};

// The hook uses useQueryClient (to invalidate after save), so it must render under
// a QueryClientProvider.
const wrapper = ({ children }: { children: ReactNode }) =>
  createElement(QueryClientProvider, { client: new QueryClient() }, children);
const render = (p = params) => renderHook(() => useInterviewQuestions(p), { wrapper });

describe('useInterviewQuestions', () => {
  beforeEach(() => {
    save.mockClear();
    vi.mocked(generateInterviewQuestions).mockClear();
    mockProvider = 'openai';
    mockHasOllamaKey = true;
  });

  it('defaults to the two earliest rounds (recruiter/HR + hiring manager)', () => {
    const { result } = render();
    expect(result.current.audiences).toEqual(['recruiter', 'hiringManager']);
  });

  it('toggleAudience adds an unselected audience and removes a selected one', () => {
    const { result } = render();

    act(() => result.current.toggleAudience('team'));
    expect(result.current.audiences).toEqual(['recruiter', 'hiringManager', 'team']);

    act(() => result.current.toggleAudience('recruiter'));
    expect(result.current.audiences).toEqual(['hiringManager', 'team']);
  });

  it('canGenerate is false once every audience is deselected (even with valid inputs)', () => {
    const { result } = render();
    expect(result.current.canGenerate).toBe(true);

    act(() => result.current.toggleAudience('recruiter'));
    act(() => result.current.toggleAudience('hiringManager'));

    expect(result.current.audiences).toEqual([]);
    expect(result.current.canGenerate).toBe(false);
  });

  it('canGenerate is false when AI is unavailable, no job description, or empty résumé', () => {
    expect(render({ ...params, canUse: false }).result.current.canGenerate).toBe(false);
    expect(render({ ...params, hasDesc: false }).result.current.canGenerate).toBe(false);
    expect(render({ ...params, resume: '   ' }).result.current.canGenerate).toBe(false);
  });

  it('passes the selected audiences to generateInterviewQuestions and persists the parsed set', async () => {
    const { result } = render();

    act(() => result.current.toggleAudience('team')); // recruiter, hiringManager, team

    await act(async () => {
      await result.current.generate();
    });

    expect(generateInterviewQuestions).toHaveBeenCalledTimes(1);
    expect(generateInterviewQuestions).toHaveBeenCalledWith(
      expect.objectContaining({ audiences: ['recruiter', 'hiringManager', 'team'] })
    );
    // Parsed questions land on state and are persisted onto the per-job record.
    expect(result.current.questions).toHaveLength(1);
    expect(save).toHaveBeenCalledWith(
      expect.objectContaining({
        jobUrl: 'https://acme.com/job/42',
        interviewQuestions: [{ id: 'iq-1', question: 'Q1', why: 'because', audience: 'recruiter' }],
      })
    );
  });

  it('flags needsResearchKey for an Ollama provider missing the web-search key', () => {
    mockProvider = 'ollama';
    mockHasOllamaKey = false;
    expect(render().result.current.needsResearchKey).toBe(true);
  });

  it('does not flag needsResearchKey for a cloud provider (uses its own web search)', () => {
    mockProvider = 'openai';
    mockHasOllamaKey = false;
    expect(render().result.current.needsResearchKey).toBe(false);
  });

  it('does not flag needsResearchKey once the Ollama key is present', () => {
    mockProvider = 'ollama';
    mockHasOllamaKey = true;
    expect(render().result.current.needsResearchKey).toBe(false);
  });

  it('does not call the generator when no audience is selected', async () => {
    const { result } = render();

    act(() => result.current.toggleAudience('recruiter'));
    act(() => result.current.toggleAudience('hiringManager'));

    await act(async () => {
      await result.current.generate();
    });

    expect(generateInterviewQuestions).not.toHaveBeenCalled();
    expect(save).not.toHaveBeenCalled();
  });
});
