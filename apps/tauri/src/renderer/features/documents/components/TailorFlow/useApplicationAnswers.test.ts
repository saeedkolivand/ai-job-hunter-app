import { createElement, type ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';

import { useApplicationAnswers } from './useApplicationAnswers';

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
const render = () => renderHook(() => useApplicationAnswers(base), { wrapper });

describe('useApplicationAnswers', () => {
  beforeEach(() => save.mockClear());

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
});
