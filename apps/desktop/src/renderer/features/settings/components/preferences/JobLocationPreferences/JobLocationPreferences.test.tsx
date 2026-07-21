/**
 * JobLocationPreferences — full-row save guard (CodeRabbit #756).
 *
 * `useSetJobPreferences` writes the WHOLE row, so spreading a not-yet-loaded
 * `jobPrefs` (undefined) would NULL every other column. The handlers must early
 * -return until the query resolves.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

const mockMutate = vi.fn();
// `undefined` models the pre-load window.
let mockJobPrefs: { location?: string } | undefined = {};

vi.mock('@/services', () => ({
  useJobPreferences: () => ({ data: mockJobPrefs }),
  useSetJobPreferences: () => ({ mutate: mockMutate }),
}));

vi.mock('@/store/preferences-store', () => ({
  useRecentLocations: () => [],
  usePreferencesStore: (selector: (s: { addRecentLocation: () => void }) => unknown) =>
    selector({ addRecentLocation: vi.fn() }),
}));

import { JobLocationPreferences } from './index';

beforeEach(() => {
  mockMutate.mockClear();
  mockJobPrefs = {};
});

describe('JobLocationPreferences — full-row save guard', () => {
  it('saves a typed location once preferences have loaded', async () => {
    const user = userEvent.setup();
    render(<JobLocationPreferences />);

    // "Nowhere City" isn't in COMMON_LOCATIONS → no autocomplete buttons, so the
    // sole button is Add.
    await user.type(screen.getByRole('textbox'), 'Nowhere City');
    await user.click(screen.getByRole('button'));

    expect(mockMutate).toHaveBeenCalledWith({ location: 'Nowhere City' });
  });

  it('does not call the full-row mutate before job preferences have loaded', async () => {
    mockJobPrefs = undefined;
    const user = userEvent.setup();
    render(<JobLocationPreferences />);

    await user.type(screen.getByRole('textbox'), 'Nowhere City');
    await user.click(screen.getByRole('button'));

    expect(mockMutate).not.toHaveBeenCalled();
  });
});
