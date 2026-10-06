import { describe, expect, it, vi } from 'vitest';

import { lookupSalaryRange, researchAnswer } from './generation';
import { installGenerationHooks, register } from './test-support';

installGenerationHooks();

describe('lookupSalaryRange (C2)', () => {
  const registerWithLookup = (lookupSalary: ReturnType<typeof vi.fn>) =>
    register({ ai: { lookupSalary } });

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
  const registerWithAnswerSearch = (answerSearch: ReturnType<typeof vi.fn>) =>
    register({ ai: { researchAnswer: answerSearch } });

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
