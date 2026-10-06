/**
 * useApplicationAnswers — selection, drafting, custom questions, rewrite/revert, guidance, language.
 * Shared `vi.mock` factories in `mocks.ts`, render helpers in `helpers.ts`.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import { generateApplicationAnswer } from '@/lib/generate';

import { generate, generateSelected, render, resetMocks } from './helpers';
import { META, save } from './mocks';

vi.mock('@/lib/generate', async () => (await import('./mocks')).generateModule);
vi.mock('@/providers/AppClientProvider', async () => (await import('./mocks')).appClientModule);

describe('useApplicationAnswers', () => {
  beforeEach(resetMocks);

  it('toggles selection and gates generation on a non-empty selection', () => {
    const { result } = render();
    expect(result.current.canGenerate).toBe(false);
    act(() => result.current.toggle('why-company'));
    expect(result.current.selected.has('why-company')).toBe(true);
    expect(result.current.canGenerate).toBe(true);
  });

  it('drafts answers and persists them linked to the job url', async () => {
    const result = await generateSelected('why-company');

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
    await generate(result);
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

    await generate(result);

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
      await generate(result);
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

      await generate(result);
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
      const result = await generateSelected('why-company');
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
      await generateSelected('salary');

      expect(generateApplicationAnswer).toHaveBeenCalledWith(
        expect.objectContaining({
          question: 'What are your salary expectations?',
          guidance: expect.stringContaining('Number:'),
        })
      );
    });

    it('omits guidance for a non-salary question', async () => {
      await generateSelected('why-company');

      expect(generateApplicationAnswer).toHaveBeenCalledWith(
        expect.objectContaining({ question: 'Why do you want to work at this company?' })
      );
      const call = vi.mocked(generateApplicationAnswer).mock.calls[0]?.[0];
      expect(call?.guidance).toBeUndefined();
    });
  });

  describe('unconfident target language is never persisted', () => {
    const guessedMeta = META;

    it('sends empty language fields when meta was seeded from an unconfident guess', async () => {
      await generateSelected('why-company', {
        meta: guessedMeta,
        targetLanguageConfident: false,
      });

      expect(save).toHaveBeenCalledWith(
        expect.objectContaining({ targetLanguage: '', resumeLanguage: '', jobAdLanguage: '' })
      );
    });

    it('still persists the real language fields when meta was confidently detected', async () => {
      await generateSelected('why-company', {
        meta: guessedMeta,
        targetLanguageConfident: true,
      });

      expect(save).toHaveBeenCalledWith(
        expect.objectContaining({ targetLanguage: 'en', resumeLanguage: 'en', jobAdLanguage: 'en' })
      );
    });

    it('persists the freshly-detected language when meta is null, even with targetLanguageConfident: false', async () => {
      // `languageIsGuess` is `!!meta && targetLanguageConfident === false` — with
      // no `meta` at all, `generate()` re-extracts its own fresh metadata (a
      // SEPARATE detection, unrelated to a caller's stale confidence flag), and
      // that detection's language fields must still reach `save`, not ''. Also
      // guards against a mutant that drops `!!meta &&` from the condition: such
      // a mutant would blank these fields purely because `targetLanguageConfident`
      // is `false` here, regardless of `meta`.
      await generateSelected('why-company', {
        meta: null,
        targetLanguageConfident: false,
      });

      expect(save).toHaveBeenCalledWith(
        expect.objectContaining({ targetLanguage: 'en', resumeLanguage: 'en', jobAdLanguage: 'en' })
      );
    });

    it('an unconfident guess survives into a rewrite re-save via updateAnswer too', async () => {
      const result = await generateSelected('why-company', {
        meta: guessedMeta,
        targetLanguageConfident: false,
      });
      save.mockClear();

      await act(async () => {
        await result.current.updateAnswer('why-company', 'Rewritten');
      });

      expect(save).toHaveBeenCalledWith(
        expect.objectContaining({ targetLanguage: '', resumeLanguage: '', jobAdLanguage: '' })
      );
    });
  });
});
