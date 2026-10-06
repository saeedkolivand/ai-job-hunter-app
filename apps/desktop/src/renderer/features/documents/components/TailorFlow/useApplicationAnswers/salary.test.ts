/**
 * useApplicationAnswers — salary: market-range lookup (C2) and scraped-range precedence (Phase 3).
 * Shared `vi.mock` factories in `mocks.ts`, render helpers in `helpers.ts`.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import { extractMetadata, generateApplicationAnswer, lookupSalaryRange } from '@/lib/generate';

import { generate, generateSelected, render, resetMocks } from './helpers';
import { META } from './mocks';

vi.mock('@/lib/generate', async () => (await import('./mocks')).generateModule);
vi.mock('@/providers/AppClientProvider', async () => (await import('./mocks')).appClientModule);

describe('useApplicationAnswers', () => {
  beforeEach(resetMocks);

  describe('salary market-range lookup (C2)', () => {
    it('triggers lookupSalaryRange for the salary question and forwards the result', async () => {
      vi.mocked(lookupSalaryRange).mockResolvedValue({ min: 65000, max: 80000, currency: 'EUR' });
      await generateSelected('salary');

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
        ...META,
        jobLocation: 'Berlin, Germany',
        jobCountry: 'DE',
      });
      vi.mocked(lookupSalaryRange).mockResolvedValue({ min: 65000, max: 80000, currency: 'EUR' });
      await generateSelected('salary');

      expect(lookupSalaryRange).toHaveBeenCalledWith(
        'Engineer',
        'Acme',
        'Berlin, Germany',
        'DE',
        'EUR'
      );
    });

    it('does not trigger lookupSalaryRange for a non-salary question', async () => {
      await generateSelected('why-company');

      expect(lookupSalaryRange).not.toHaveBeenCalled();
      const call = vi.mocked(generateApplicationAnswer).mock.calls[0]?.[0];
      expect(call?.salaryRange).toBeUndefined();
    });

    it('degrades to the C1 fallback (undefined salaryRange, no throw) when the lookup fails', async () => {
      vi.mocked(lookupSalaryRange).mockRejectedValue(new Error('provider unavailable'));
      const { result } = render();
      act(() => result.current.toggle('salary'));

      // `generate()` must not throw even though the lookup rejects.
      await generate(result);

      expect(result.current.error).toBeNull();
      const call = vi.mocked(generateApplicationAnswer).mock.calls[0]?.[0];
      expect(call?.salaryRange).toBeUndefined();
    });
  });

  describe('scraped salary precedence (Phase 3)', () => {
    it('a complete scraped range wins: skips lookupSalaryRange and grounds the answer in it', async () => {
      await generateSelected('salary', {
        salaryMin: 70000,
        salaryMax: 90000,
        salaryCurrency: 'EUR',
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
        await generateSelected('salary', overrides);

        expect(lookupSalaryRange).toHaveBeenCalledTimes(1);
        expect(generateApplicationAnswer).toHaveBeenCalledWith(
          expect.objectContaining({ salaryRange: { min: 65000, max: 80000, currency: 'EUR' } })
        );
      }
    );

    it('rounds a decimal scraped range to integers (parity with the web path)', async () => {
      await generateSelected('salary', {
        salaryMin: 70000.4,
        salaryMax: 89999.6,
        salaryCurrency: 'EUR',
      });

      expect(lookupSalaryRange).not.toHaveBeenCalled();
      expect(generateApplicationAnswer).toHaveBeenCalledWith(
        expect.objectContaining({ salaryRange: { min: 70000, max: 90000, currency: 'EUR' } })
      );
    });

    it('normalizes a lowercase/mixed-case scraped currency to uppercase', async () => {
      await generateSelected('salary', {
        salaryMin: 70000,
        salaryMax: 90000,
        salaryCurrency: 'Usd',
      });

      expect(lookupSalaryRange).not.toHaveBeenCalled();
      expect(generateApplicationAnswer).toHaveBeenCalledWith(
        expect.objectContaining({ salaryRange: { min: 70000, max: 90000, currency: 'USD' } })
      );
    });

    it('a scraped range never leaks into a non-salary question', async () => {
      await generateSelected('why-company', {
        salaryMin: 70000,
        salaryMax: 90000,
        salaryCurrency: 'EUR',
      });

      expect(lookupSalaryRange).not.toHaveBeenCalled();
      const call = vi.mocked(generateApplicationAnswer).mock.calls[0]?.[0];
      expect(call?.salaryRange).toBeUndefined();
    });
  });
});
