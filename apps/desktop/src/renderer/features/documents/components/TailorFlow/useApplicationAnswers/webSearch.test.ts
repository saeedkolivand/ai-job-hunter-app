/**
 * useApplicationAnswers — opt-in per-question web search and its fan-out cap.
 * Shared `vi.mock` factories in `mocks.ts`, render helpers in `helpers.ts`.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import { APPLICATION_QUESTIONS } from '@ajh/prompts/generate';

import { generateApplicationAnswer, researchAnswer } from '@/lib/generate';

import { WEB_SEARCH_MAX_PER_RUN } from '../useApplicationAnswers';
import { generate, render, resetMocks } from './helpers';

vi.mock('@/lib/generate', async () => (await import('./mocks')).generateModule);
vi.mock('@/providers/AppClientProvider', async () => (await import('./mocks')).appClientModule);

describe('useApplicationAnswers', () => {
  beforeEach(resetMocks);

  describe('opt-in per-question web search', () => {
    it('defaults to off — no search call, and generation proceeds unchanged', async () => {
      const { result } = render();
      expect(result.current.searchWeb).toBe(false);
      act(() => result.current.toggle('why-company'));

      await generate(result);

      expect(researchAnswer).not.toHaveBeenCalled();
      const call = vi.mocked(generateApplicationAnswer).mock.calls[0]?.[0];
      expect(call?.webSearchNotes).toBe('');
    });

    it('when on, fetches notes per question and forwards them to the answer generator', async () => {
      vi.mocked(researchAnswer).mockResolvedValue('Acme raised a Series B in 2026.');
      const { result } = render();
      act(() => result.current.setSearchWeb(true));
      act(() => result.current.toggle('why-company'));

      await generate(result);

      expect(researchAnswer).toHaveBeenCalledWith(
        'Why do you want to work at this company?',
        'Engineer',
        'Acme'
      );
      expect(generateApplicationAnswer).toHaveBeenCalledWith(
        expect.objectContaining({ webSearchNotes: 'Acme raised a Series B in 2026.' })
      );
    });

    it('degrades to an empty string (answer still generates) when the search fails', async () => {
      vi.mocked(researchAnswer).mockRejectedValue(new Error('provider cannot search'));
      const { result } = render();
      act(() => result.current.setSearchWeb(true));
      act(() => result.current.toggle('why-company'));

      await expect(
        act(async () => {
          await result.current.generate();
        })
      ).resolves.not.toThrow();

      expect(result.current.error).toBeNull();
      // The loop must CONTINUE past the caught rejection and still produce an
      // answer with no web grounding — a regression that short-circuits the
      // loop after a search failure would leave this answer missing/empty
      // instead of the mocked deterministic text.
      expect(result.current.answers['why-company']).toContain('payments migration');
      expect(generateApplicationAnswer).toHaveBeenCalledWith(
        expect.objectContaining({ webSearchNotes: '' })
      );
    });
  });

  describe('web-search fan-out cap', () => {
    it('caps per-question searches at WEB_SEARCH_MAX_PER_RUN; the rest still answer without web grounding', async () => {
      // The registry alone must exceed the cap for this test to be meaningful.
      expect(APPLICATION_QUESTIONS.length).toBeGreaterThan(WEB_SEARCH_MAX_PER_RUN);
      vi.mocked(researchAnswer).mockResolvedValue('Acme raised a Series B in 2026.');
      const { result } = render();
      act(() => result.current.setSearchWeb(true));
      act(() => {
        for (const q of APPLICATION_QUESTIONS) result.current.toggle(q.id);
      });

      await generate(result);

      expect(researchAnswer).toHaveBeenCalledTimes(WEB_SEARCH_MAX_PER_RUN);
      // The loop never short-circuits — every selected question still got an answer.
      expect(Object.keys(result.current.answers)).toHaveLength(APPLICATION_QUESTIONS.length);
      // Everything past the cap generated WITHOUT web grounding.
      const uncappedCalls = vi
        .mocked(generateApplicationAnswer)
        .mock.calls.slice(WEB_SEARCH_MAX_PER_RUN);
      expect(uncappedCalls.length).toBeGreaterThan(0);
      for (const [args] of uncappedCalls) {
        expect(args.webSearchNotes).toBe('');
      }
    });
  });
});
