/**
 * JobLocationPreferences — save payload shape.
 *
 * Two things this pins:
 *  1. pre-load guard (CodeRabbit #756) — the handlers early-return until the
 *     query resolves, so no edit is saved off a row the user cannot see;
 *  2. clearing (this branch) — `jobPreferences.set` MERGES over the stored row,
 *     so a cleared field must travel as an explicit `null`. `undefined` is
 *     dropped by `JSON.stringify` and arrives as an omitted key, which the merge
 *     KEEPS — the remove then no-ops while still answering `{"success": true}`.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

const mockMutate = vi.fn();
// `undefined` models the pre-load window.
let mockJobPrefs: { location?: string; countryCode?: string } | undefined = {};

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

describe('JobLocationPreferences — clearing the saved location', () => {
  it('sends explicit nulls for location and countryCode that survive JSON serialization', async () => {
    mockJobPrefs = { location: 'Berlin, Germany', countryCode: 'de' };
    const user = userEvent.setup();
    render(<JobLocationPreferences />);

    await user.click(screen.getByRole('button', { name: /remove location/i }));

    expect(mockMutate).toHaveBeenCalledWith({ location: null, countryCode: null });

    // Assert the SERIALIZED payload too: the Tauri invoke JSON-encodes it, and
    // `JSON.stringify` drops undefined keys — so reverting either field to
    // `undefined` (the regression) leaves the key out entirely, the backend
    // merge keeps the stored value, and the remove silently no-ops.
    const call = mockMutate.mock.calls.at(0);
    if (!call) throw new Error('expected mockMutate to have been called');
    const payload: unknown = call[0];
    expect(JSON.parse(JSON.stringify(payload))).toEqual({ location: null, countryCode: null });
  });
});
