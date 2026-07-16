import { createElement, type ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';

import { APPLICATION_QUESTIONS } from '@ajh/prompts/generate';

import {
  extractMetadata,
  generateApplicationAnswer,
  lookupSalaryRange,
  researchAnswer,
} from '@/lib/generate';

import { useApplicationAnswers, WEB_SEARCH_MAX_PER_RUN } from './useApplicationAnswers';

// Stub the generation lib: metadata + one deterministic answer, no research.
vi.mock('@/lib/generate', () => ({
  extractMetadata: vi.fn().mockResolvedValue({
    candidateName: 'Jane',
    jobTitle: 'Engineer',
    companyName: 'Acme',
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    mismatch: false,
    targetLanguage: 'en',
    topRequirements: [],
  }),
  generateApplicationAnswer: vi.fn().mockResolvedValue('Because I led a payments migration.'),
  researchCompany: vi.fn().mockResolvedValue(''),
  researchAnswer: vi.fn().mockResolvedValue(''),
  lookupSalaryRange: vi.fn().mockResolvedValue(undefined),
}));

const save = vi.fn().mockResolvedValue({ id: 'gen-1', success: true });
vi.mock('@/providers/AppClientProvider', () => ({
  useAppClient: () => ({ aiGenerations: { save } }),
}));

const base = {
  resume: 'My resume',
  jobDesc: 'Backend role at Acme',
  model: 'llama3',
  researchCompany: false,
  meta: null,
  canUse: true,
  hasDesc: true,
  jobUrl: 'https://acme.com/job/1',
  board: 'linkedin',
};

const wrapper = ({ children }: { children: ReactNode }) =>
  createElement(QueryClientProvider, { client: new QueryClient() }, children);
const render = (overrides: Partial<Parameters<typeof useApplicationAnswers>[0]> = {}) =>
  renderHook(() => useApplicationAnswers({ ...base, ...overrides }), { wrapper });

describe('useApplicationAnswers', () => {
  beforeEach(() => {
    save.mockClear();
    vi.mocked(generateApplicationAnswer).mockClear();
    vi.mocked(lookupSalaryRange).mockClear();
    vi.mocked(lookupSalaryRange).mockResolvedValue(undefined);
    vi.mocked(researchAnswer).mockClear();
    vi.mocked(researchAnswer).mockResolvedValue('');
  });

  it('toggles selection and gates generation on a non-empty selection', () => {
    const { result } = render();
    expect(result.current.canGenerate).toBe(false);
    act(() => result.current.toggle('why-company'));
    expect(result.current.selected.has('why-company')).toBe(true);
    expect(result.current.canGenerate).toBe(true);
  });

  it('drafts answers and persists them linked to the job url', async () => {
    const { result } = render();
    act(() => result.current.toggle('why-company'));

    await act(async () => {
      await result.current.generate();
    });

    expect(result.current.answers['why-company']).toContain('payments migration');
    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith(
      expect.objectContaining({
        jobUrl: 'https://acme.com/job/1',
        board: 'linkedin',
        applicationAnswers: [
          expect.objectContaining({
            id: 'why-company',
            answer: 'Because I led a payments migration.',
          }),
        ],
      })
    );
  });

  it('does nothing when nothing is selected', async () => {
    const { result } = render();
    await act(async () => {
      await result.current.generate();
    });
    expect(save).not.toHaveBeenCalled();
  });

  it('appends a trimmed custom question and ignores empty input', () => {
    const { result } = render();
    act(() => result.current.addCustom('  How do you handle conflict?  '));
    act(() => result.current.addCustom('   '));
    expect(result.current.custom).toHaveLength(1);
    expect(result.current.custom[0]?.question).toBe('How do you handle conflict?');
  });

  it('gates generation true with only a custom question', () => {
    const { result } = render();
    expect(result.current.canGenerate).toBe(false);
    act(() => result.current.addCustom('Why this team?'));
    expect(result.current.selected.size).toBe(0);
    expect(result.current.canGenerate).toBe(true);
  });

  it('flows a custom question through generate into persisted answers', async () => {
    const { result } = render();
    act(() => result.current.addCustom('What excites you about this role?'));
    const customId = result.current.custom[0]?.id ?? '';

    await act(async () => {
      await result.current.generate();
    });

    expect(customId).not.toBe('');
    expect(result.current.answers[customId]).toContain('payments migration');
    expect(save).toHaveBeenCalledWith(
      expect.objectContaining({
        applicationAnswers: [
          expect.objectContaining({
            id: customId,
            question: 'What excites you about this role?',
            answer: 'Because I led a payments migration.',
          }),
        ],
      })
    );
  });

  it('removes a custom question by id', () => {
    const { result } = render();
    act(() => result.current.addCustom('Keep me'));
    act(() => result.current.addCustom('Drop me'));
    const dropId = result.current.custom[1]?.id ?? '';
    act(() => result.current.removeCustom(dropId));
    expect(result.current.custom).toHaveLength(1);
    expect(result.current.custom[0]?.question).toBe('Keep me');
  });

  describe('updateAnswer', () => {
    it('is a no-op before the first generate (no save context yet)', async () => {
      const { result } = render();
      // No generate() call — lastSaveContextRef is null.
      await act(async () => {
        await result.current.updateAnswer('why-company', 'New text');
      });
      expect(save).not.toHaveBeenCalled();
    });

    it('updates state + persists the FULL answer set after a rewrite', async () => {
      const { result } = render();
      // Select a predefined question and add a custom one.
      act(() => result.current.toggle('why-company'));
      act(() => result.current.addCustom('What excites you about this role?'));
      const customId = result.current.custom[0]?.id ?? '';
      expect(customId).not.toBe('');

      // Generate both answers (save is called once here).
      await act(async () => {
        await result.current.generate();
      });
      expect(save).toHaveBeenCalledTimes(1);
      save.mockClear();

      // Rewrite only the predefined answer.
      await act(async () => {
        await result.current.updateAnswer('why-company', 'Rewritten predefined answer');
      });

      // State updated for the rewritten answer.
      expect(result.current.answers['why-company']).toBe('Rewritten predefined answer');

      // Save called once with the FULL set: rewritten predefined + untouched custom.
      expect(save).toHaveBeenCalledTimes(1);
      expect(save).toHaveBeenCalledWith(
        expect.objectContaining({
          applicationAnswers: expect.arrayContaining([
            expect.objectContaining({
              id: 'why-company',
              answer: 'Rewritten predefined answer',
            }),
            expect.objectContaining({
              id: customId,
              question: 'What excites you about this role?',
              answer: 'Because I led a payments migration.',
            }),
          ]),
        })
      );
    });

    it('untouched answers survive a rewrite (not dropped from the persisted set)', async () => {
      const { result } = render();
      act(() => result.current.toggle('why-company'));
      act(() => result.current.addCustom('Untouched question'));
      const customId = result.current.custom[0]?.id ?? '';

      await act(async () => {
        await result.current.generate();
      });
      save.mockClear();

      await act(async () => {
        await result.current.updateAnswer('why-company', 'Only this changed');
      });

      const call = save.mock.calls[0]?.[0] as { applicationAnswers: { id: string }[] } | undefined;
      const savedIds = call?.applicationAnswers.map((a) => a.id) ?? [];
      expect(savedIds).toContain('why-company');
      expect(savedIds).toContain(customId);
    });
  });

  describe('revertAnswer', () => {
    it('restores state to a previous value WITHOUT calling save', async () => {
      const { result } = render();
      act(() => result.current.toggle('why-company'));

      await act(async () => {
        await result.current.generate();
      });
      save.mockClear();

      // Verify the generated answer is present.
      expect(result.current.answers['why-company']).toContain('payments migration');

      // Revert to a known previous text.
      act(() => result.current.revertAnswer('why-company', 'Old text before rewrite'));

      expect(result.current.answers['why-company']).toBe('Old text before rewrite');
      // No save triggered — revert is local-only.
      expect(save).not.toHaveBeenCalled();
    });
  });

  describe('guidance forwarding', () => {
    it('forwards the registry guidance for the salary question', async () => {
      const { result } = render();
      act(() => result.current.toggle('salary'));

      await act(async () => {
        await result.current.generate();
      });

      expect(generateApplicationAnswer).toHaveBeenCalledWith(
        expect.objectContaining({
          question: 'What are your salary expectations?',
          guidance: expect.stringContaining('Number:'),
        })
      );
    });

    it('omits guidance for a non-salary question', async () => {
      const { result } = render();
      act(() => result.current.toggle('why-company'));

      await act(async () => {
        await result.current.generate();
      });

      expect(generateApplicationAnswer).toHaveBeenCalledWith(
        expect.objectContaining({ question: 'Why do you want to work at this company?' })
      );
      const call = vi.mocked(generateApplicationAnswer).mock.calls[0]?.[0];
      expect(call?.guidance).toBeUndefined();
    });
  });

  describe('salary market-range lookup (C2)', () => {
    it('triggers lookupSalaryRange for the salary question and forwards the result', async () => {
      vi.mocked(lookupSalaryRange).mockResolvedValue({ min: 65000, max: 80000, currency: 'EUR' });
      const { result } = render();
      act(() => result.current.toggle('salary'));

      await act(async () => {
        await result.current.generate();
      });

      // No jobCountry in the base mock → country/currency both undefined (today's
      // unconstrained behavior — the unknown-country fallback). `model` is no
      // longer threaded (routing is backend-owned, task #16).
      expect(lookupSalaryRange).toHaveBeenCalledWith('Engineer', 'Acme', '', undefined, undefined);
      expect(generateApplicationAnswer).toHaveBeenCalledWith(
        expect.objectContaining({
          question: 'What are your salary expectations?',
          salaryRange: { min: 65000, max: 80000, currency: 'EUR' },
        })
      );
    });

    it('grounds the lookup in the detected job country + its currency (currency-grounding fix)', async () => {
      vi.mocked(extractMetadata).mockResolvedValueOnce({
        candidateName: 'Jane',
        jobTitle: 'Engineer',
        companyName: 'Acme',
        resumeLanguage: 'en',
        jobAdLanguage: 'en',
        mismatch: false,
        targetLanguage: 'en',
        topRequirements: [],
        jobLocation: 'Berlin, Germany',
        jobCountry: 'DE',
      });
      vi.mocked(lookupSalaryRange).mockResolvedValue({ min: 65000, max: 80000, currency: 'EUR' });
      const { result } = render();
      act(() => result.current.toggle('salary'));

      await act(async () => {
        await result.current.generate();
      });

      expect(lookupSalaryRange).toHaveBeenCalledWith(
        'Engineer',
        'Acme',
        'Berlin, Germany',
        'DE',
        'EUR'
      );
    });

    it('does not trigger lookupSalaryRange for a non-salary question', async () => {
      const { result } = render();
      act(() => result.current.toggle('why-company'));

      await act(async () => {
        await result.current.generate();
      });

      expect(lookupSalaryRange).not.toHaveBeenCalled();
      const call = vi.mocked(generateApplicationAnswer).mock.calls[0]?.[0];
      expect(call?.salaryRange).toBeUndefined();
    });

    it('degrades to the C1 fallback (undefined salaryRange, no throw) when the lookup fails', async () => {
      vi.mocked(lookupSalaryRange).mockRejectedValue(new Error('provider unavailable'));
      const { result } = render();
      act(() => result.current.toggle('salary'));

      // `generate()` must not throw even though the lookup rejects.
      await act(async () => {
        await result.current.generate();
      });

      expect(result.current.error).toBeNull();
      const call = vi.mocked(generateApplicationAnswer).mock.calls[0]?.[0];
      expect(call?.salaryRange).toBeUndefined();
    });
  });

  describe('scraped salary precedence (Phase 3)', () => {
    it('a complete scraped range wins: skips lookupSalaryRange and grounds the answer in it', async () => {
      const { result } = render({ salaryMin: 70000, salaryMax: 90000, salaryCurrency: 'EUR' });
      act(() => result.current.toggle('salary'));

      await act(async () => {
        await result.current.generate();
      });

      expect(lookupSalaryRange).not.toHaveBeenCalled();
      expect(generateApplicationAnswer).toHaveBeenCalledWith(
        expect.objectContaining({
          question: 'What are your salary expectations?',
          salaryRange: { min: 70000, max: 90000, currency: 'EUR' },
        })
      );
    });

    it.each([
      ['missing currency', { salaryMin: 70000, salaryMax: 90000, salaryCurrency: undefined }],
      ['min greater than max', { salaryMin: 90000, salaryMax: 70000, salaryCurrency: 'EUR' }],
      ['malformed currency shape', { salaryMin: 70000, salaryMax: 90000, salaryCurrency: 'E1' }],
      // A real Adzuna shape ("up to X" postings report min: 0) — the prompt
      // layer's buildSalaryRangeBlock treats a non-positive bound as invalid
      // and renders an EMPTY block, so this MUST fall through to the web
      // lookup rather than being accepted here and silently losing the range.
      ['min is zero (non-positive)', { salaryMin: 0, salaryMax: 90000, salaryCurrency: 'EUR' }],
      [
        'max is negative (non-positive)',
        { salaryMin: 70000, salaryMax: -1, salaryCurrency: 'EUR' },
      ],
      ['min is non-finite', { salaryMin: Infinity, salaryMax: 90000, salaryCurrency: 'EUR' }],
      // Rounds BEFORE validating: a raw min in (0, 0.5) is > 0 but rounds to
      // 0, so it must still be rejected here (not just at the prompt layer).
      ['min rounds down to zero', { salaryMin: 0.4, salaryMax: 90000, salaryCurrency: 'EUR' }],
    ])(
      'falls back to the web lookup on a partial/invalid scraped range (%s)',
      async (_label, overrides) => {
        vi.mocked(lookupSalaryRange).mockResolvedValue({ min: 65000, max: 80000, currency: 'EUR' });
        const { result } = render(overrides);
        act(() => result.current.toggle('salary'));

        await act(async () => {
          await result.current.generate();
        });

        expect(lookupSalaryRange).toHaveBeenCalledTimes(1);
        expect(generateApplicationAnswer).toHaveBeenCalledWith(
          expect.objectContaining({ salaryRange: { min: 65000, max: 80000, currency: 'EUR' } })
        );
      }
    );

    it('rounds a decimal scraped range to integers (parity with the web path)', async () => {
      const { result } = render({ salaryMin: 70000.4, salaryMax: 89999.6, salaryCurrency: 'EUR' });
      act(() => result.current.toggle('salary'));

      await act(async () => {
        await result.current.generate();
      });

      expect(lookupSalaryRange).not.toHaveBeenCalled();
      expect(generateApplicationAnswer).toHaveBeenCalledWith(
        expect.objectContaining({ salaryRange: { min: 70000, max: 90000, currency: 'EUR' } })
      );
    });

    it('normalizes a lowercase/mixed-case scraped currency to uppercase', async () => {
      const { result } = render({ salaryMin: 70000, salaryMax: 90000, salaryCurrency: 'Usd' });
      act(() => result.current.toggle('salary'));

      await act(async () => {
        await result.current.generate();
      });

      expect(lookupSalaryRange).not.toHaveBeenCalled();
      expect(generateApplicationAnswer).toHaveBeenCalledWith(
        expect.objectContaining({ salaryRange: { min: 70000, max: 90000, currency: 'USD' } })
      );
    });

    it('a scraped range never leaks into a non-salary question', async () => {
      const { result } = render({ salaryMin: 70000, salaryMax: 90000, salaryCurrency: 'EUR' });
      act(() => result.current.toggle('why-company'));

      await act(async () => {
        await result.current.generate();
      });

      expect(lookupSalaryRange).not.toHaveBeenCalled();
      const call = vi.mocked(generateApplicationAnswer).mock.calls[0]?.[0];
      expect(call?.salaryRange).toBeUndefined();
    });
  });

  describe('opt-in per-question web search', () => {
    it('defaults to off — no search call, and generation proceeds unchanged', async () => {
      const { result } = render();
      expect(result.current.searchWeb).toBe(false);
      act(() => result.current.toggle('why-company'));

      await act(async () => {
        await result.current.generate();
      });

      expect(researchAnswer).not.toHaveBeenCalled();
      const call = vi.mocked(generateApplicationAnswer).mock.calls[0]?.[0];
      expect(call?.webSearchNotes).toBe('');
    });

    it('when on, fetches notes per question and forwards them to the answer generator', async () => {
      vi.mocked(researchAnswer).mockResolvedValue('Acme raised a Series B in 2026.');
      const { result } = render();
      act(() => result.current.setSearchWeb(true));
      act(() => result.current.toggle('why-company'));

      await act(async () => {
        await result.current.generate();
      });

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

      await act(async () => {
        await result.current.generate();
      });

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
