import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

// Stub the generation pipeline. `parseLikelyQuestions`/`parseStarFeedback` return
// fixed values so we can assert they land on state; the generate* functions are
// the seam we assert params flow into.
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
  generateLikelyInterviewQuestions: vi.fn().mockResolvedValue('RAW_QUESTIONS'),
  generateStarFeedback: vi.fn().mockImplementation(async ({ onToken }) => {
    onToken?.('partial ');
    onToken?.('feedback');
    return 'RAW_FEEDBACK';
  }),
  parseLikelyQuestions: vi
    .fn()
    .mockReturnValue([{ id: 'lq-1', question: 'Tell me about a time…', type: 'behavioral' }]),
  parseStarFeedback: vi.fn().mockReturnValue({
    strengths: ['Clear'],
    gaps: [],
    star: { situation: true, task: true, action: true, result: true },
    rewrite: 'Tightened answer.',
  }),
}));

import {
  extractMetadata,
  generateLikelyInterviewQuestions,
  generateStarFeedback,
  type LikelyQuestion,
} from '@/lib/generate';

import { useInterviewPractice } from './use-interview-practice';

const params = {
  resume: 'my resume',
  jobDesc: 'JD',
  model: 'llama',
  canUse: true,
  hasDesc: true,
};

const render = (p = params) => renderHook(() => useInterviewPractice(p));

describe('useInterviewPractice', () => {
  beforeEach(() => {
    vi.mocked(extractMetadata).mockClear();
    vi.mocked(generateLikelyInterviewQuestions).mockClear();
    vi.mocked(generateStarFeedback).mockClear();
  });

  it('canGenerate is false when AI is unavailable, no job description, or empty résumé', () => {
    expect(render({ ...params, canUse: false }).result.current.canGenerate).toBe(false);
    expect(render({ ...params, hasDesc: false }).result.current.canGenerate).toBe(false);
    expect(render({ ...params, resume: '   ' }).result.current.canGenerate).toBe(false);
  });

  it('canGenerate is true with valid inputs', () => {
    expect(render().result.current.canGenerate).toBe(true);
  });

  it('generate() extracts metadata, generates likely questions, and parses them onto state', async () => {
    const { result } = render();

    await act(async () => {
      await result.current.generate();
    });

    expect(extractMetadata).toHaveBeenCalledWith('my resume', 'JD', 'llama');
    expect(generateLikelyInterviewQuestions).toHaveBeenCalledWith(
      expect.objectContaining({ resume: 'my resume', jobAd: 'JD', model: 'llama' })
    );
    // Ids are the parser's positional id stamped with a per-generation nonce
    // (see the "Regenerate" describe block below for why).
    expect(result.current.questions).toEqual([
      { id: '1-lq-1', question: 'Tell me about a time…', type: 'behavioral' },
    ]);
  });

  it('generate() surfaces an error message and leaves questions empty when metadata extraction fails', async () => {
    vi.mocked(extractMetadata).mockRejectedValueOnce(new Error('extract boom'));
    const { result } = render();

    await act(async () => {
      await result.current.generate();
    });

    expect(result.current.error).toBe('extract boom');
    expect(result.current.questions).toEqual([]);
    expect(result.current.generating).toBe(false);
    expect(generateLikelyInterviewQuestions).not.toHaveBeenCalled();
  });

  it('generate() surfaces an error message when the question-generation call itself fails', async () => {
    vi.mocked(generateLikelyInterviewQuestions).mockRejectedValueOnce(new Error('generate boom'));
    const { result } = render();

    await act(async () => {
      await result.current.generate();
    });

    expect(result.current.error).toBe('generate boom');
    expect(result.current.questions).toEqual([]);
    expect(result.current.generating).toBe(false);
  });

  it('does not call the generator when canGenerate is false', async () => {
    const { result } = render({ ...params, hasDesc: false });

    await act(async () => {
      await result.current.generate();
    });

    expect(generateLikelyInterviewQuestions).not.toHaveBeenCalled();
    expect(result.current.questions).toEqual([]);
  });

  it('getFeedback() is a no-op before a question set has been generated (no metadata yet)', async () => {
    const { result } = render();

    await act(async () => {
      await result.current.getFeedback(
        { id: 'lq-1', question: 'Q?', type: 'behavioral' },
        'my answer'
      );
    });

    expect(generateStarFeedback).not.toHaveBeenCalled();
    expect(result.current.feedback['lq-1']).toBeUndefined();
  });

  it('getFeedback() is a no-op for a blank answer', async () => {
    const { result } = render();
    await act(async () => {
      await result.current.generate();
    });

    await act(async () => {
      await result.current.getFeedback({ id: 'lq-1', question: 'Q?', type: 'behavioral' }, '   ');
    });

    expect(generateStarFeedback).not.toHaveBeenCalled();
  });

  it('getFeedback() streams tokens into feedback.text, then lands the parsed rubric', async () => {
    const { result } = render();
    await act(async () => {
      await result.current.generate();
    });

    await act(async () => {
      await result.current.getFeedback(
        { id: 'lq-1', question: 'Q?', type: 'behavioral' },
        'my answer'
      );
    });

    expect(generateStarFeedback).toHaveBeenCalledWith(
      expect.objectContaining({ question: 'Q?', answer: 'my answer', model: 'llama' })
    );
    const entry = result.current.feedback['lq-1'];
    expect(entry?.loading).toBe(false);
    expect(entry?.error).toBeNull();
    expect(entry?.feedback).toEqual({
      strengths: ['Clear'],
      gaps: [],
      star: { situation: true, task: true, action: true, result: true },
      rewrite: 'Tightened answer.',
    });
  });

  it('getFeedback() surfaces an error message on failure', async () => {
    vi.mocked(generateStarFeedback).mockRejectedValueOnce(new Error('boom'));
    const { result } = render();
    await act(async () => {
      await result.current.generate();
    });

    await act(async () => {
      await result.current.getFeedback(
        { id: 'lq-1', question: 'Q?', type: 'behavioral' },
        'my answer'
      );
    });

    const entry = result.current.feedback['lq-1'];
    expect(entry?.loading).toBe(false);
    expect(entry?.error).toBe('boom');
    expect(entry?.feedback).toBeNull();
  });

  describe('Regenerate — stale id / stale stream safety', () => {
    it('stamps a per-generation nonce so two Regenerate calls never produce colliding ids', async () => {
      const { result } = render();

      await act(async () => {
        await result.current.generate();
      });
      const firstId = result.current.questions[0]?.id;

      await act(async () => {
        await result.current.generate();
      });
      const secondId = result.current.questions[0]?.id;

      expect(firstId).toBeDefined();
      expect(secondId).toBeDefined();
      expect(secondId).not.toBe(firstId);
    });

    it('aborts an outstanding feedback stream on Regenerate — late tokens/result never land on the new question set', async () => {
      const { result } = render();
      await act(async () => {
        await result.current.generate();
      });
      const gen1Question = result.current.questions[0] as LikelyQuestion;

      // Simulate a STAR request still in flight: capture the signal + a manual
      // resolver instead of letting the mock resolve immediately.
      let capturedSignal: AbortSignal | undefined;
      let resolveFeedback: (v: string) => void = () => {};
      vi.mocked(generateStarFeedback).mockImplementationOnce(
        ({ signal }) =>
          new Promise<string>((resolve) => {
            capturedSignal = signal;
            resolveFeedback = resolve;
          })
      );

      act(() => {
        void result.current.getFeedback(gen1Question, 'my answer');
      });
      expect(result.current.feedback[gen1Question.id]?.loading).toBe(true);

      // Regenerate while the feedback request above is still pending — this
      // must abort it before producing the new question set.
      await act(async () => {
        await result.current.generate();
      });
      const gen2Question = result.current.questions[0] as LikelyQuestion;
      expect(gen2Question.id).not.toBe(gen1Question.id);
      expect(capturedSignal?.aborted).toBe(true);

      // The stale stream "arrives" late (tokens, then the final result) — none
      // of it may land on the fresh state.
      await act(async () => {
        resolveFeedback('LATE_RAW_FEEDBACK');
        await Promise.resolve();
      });

      expect(result.current.feedback[gen2Question.id]).toBeUndefined();
      expect(result.current.feedback[gen1Question.id]).toBeUndefined();
    });
  });
});
