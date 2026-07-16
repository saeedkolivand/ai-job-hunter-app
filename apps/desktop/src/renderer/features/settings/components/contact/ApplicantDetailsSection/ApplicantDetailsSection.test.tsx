/**
 * ApplicantDetailsSection — the salaryExpectation write-through (Task #30).
 *
 * Covers:
 *  1. Editing the salary field ALSO pushes it onto the backend job_preferences
 *     store (merged with whatever else is already there).
 *  2. Editing a DIFFERENT applicant field (e.g. notice period) does NOT push to
 *     job_preferences — the write-through is scoped to salaryExpectation only.
 *  3. Clearing the salary field pushes `undefined`, not an empty string.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// ── service mock ──────────────────────────────────────────────────────────────
// Must be hoisted before the component import so the factory runs first.

const mockMutate = vi.fn();
let mockJobPrefs: { location?: string; salaryExpectation?: string } = {};

vi.mock('@/services', () => ({
  useJobPreferences: () => ({ data: mockJobPrefs }),
  useSetJobPreferences: () => ({ mutate: mockMutate }),
}));

// ── import component + store AFTER mocks ──────────────────────────────────────

import { usePreferencesStore } from '@/store/preferences-store';

import { ApplicantDetailsSection } from './index';

beforeEach(() => {
  mockMutate.mockClear();
  mockJobPrefs = {};
  usePreferencesStore.getState().setApplicant(undefined);
});

describe('ApplicantDetailsSection — salaryExpectation write-through', () => {
  it('pushes the edited salary onto job_preferences, merged with existing backend fields', async () => {
    mockJobPrefs = { location: 'Berlin' };
    const user = userEvent.setup();
    render(<ApplicantDetailsSection />);

    const salaryInput = screen.getByLabelText('Salary expectation');
    await user.type(salaryInput, '€75,000');

    expect(mockMutate).toHaveBeenCalled();
    const lastCall = mockMutate.mock.calls.at(-1);
    if (!lastCall) throw new Error('expected mockMutate to have been called');
    // `type()` fires one change event per keystroke; the LAST call carries the
    // full typed string merged onto the pre-existing backend `location`.
    expect(lastCall[0]).toMatchObject({ location: 'Berlin', salaryExpectation: '€75,000' });
  });

  it('does NOT push to job_preferences when editing a different applicant field', async () => {
    const user = userEvent.setup();
    render(<ApplicantDetailsSection />);

    const noticeInput = screen.getByLabelText('Notice period');
    await user.type(noticeInput, '2 weeks');

    expect(mockMutate).not.toHaveBeenCalled();
  });

  it('clears the backend salaryExpectation to undefined (not an empty string) when the field is emptied', async () => {
    usePreferencesStore.getState().setApplicant({ salaryExpectation: '€75,000' });
    const user = userEvent.setup();
    render(<ApplicantDetailsSection />);

    const salaryInput = screen.getByLabelText('Salary expectation');
    await user.clear(salaryInput);
    // `clear()` fires a single change event with the final empty value — no
    // intermediate keystrokes to assert on, so a synchronous check suffices.
    expect(mockMutate).toHaveBeenCalledWith(
      expect.objectContaining({ salaryExpectation: undefined })
    );
  });
});
