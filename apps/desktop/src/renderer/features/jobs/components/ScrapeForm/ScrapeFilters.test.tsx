/**
 * ScrapeFilters — location field ownership of the picked-suggestion payload.
 *
 * The bug this pins: after picking "Berlin, Germany" (countryCode/lat/lon
 * captured) the user types over it — e.g. "Amsterdam". The typed text is NOT a
 * pick, so the stale structured payload must be dropped: otherwise the scrape
 * runs against the old market and the backend's geocode backfill (which only
 * fires when `countryCode` is absent) never runs. Mirrors autopilot StepTarget.
 *
 * `LocationInput` is stubbed down to a plain controlled field: its own picker
 * behavior (portal dropdown, debounced geocode) is covered by packages/ui's
 * LocationInput.test.tsx — what matters here is which payload ScrapeFilters
 * forwards to `onFormChange`.
 */
import type { ComponentProps } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import type * as AjhUi from '@ajh/ui';

import type { ScrapeFormState } from './constants';

// i18n: identity t() so labels are stable keys.
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// Stub ONLY LocationInput; every other @ajh/ui primitive stays real.
vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  const LocationInputStub = ({
    value,
    onChange,
    onSelectSuggestion,
  }: ComponentProps<typeof actual.LocationInput>) => (
    <>
      <actual.Input
        data-testid="location-input"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
      <actual.Button
        onClick={() => {
          // Fire in the REAL order LocationInput.select() uses: onChange(display)
          // FIRST, then onSelectSuggestion(s). The ordering matters — if the
          // clearing onChange ran last it would wipe the structured payload the
          // pick just captured, and a stub that only fires onSelectSuggestion
          // could never catch that.
          onChange('Berlin, Germany');
          onSelectSuggestion?.({
            display: 'Berlin, Germany',
            countryCode: 'DE',
            lat: 52.52,
            lon: 13.4,
          });
        }}
      >
        pick
      </actual.Button>
    </>
  );
  return { ...actual, LocationInput: LocationInputStub };
});

import { ScrapeFilters } from './ScrapeFilters';

function buildForm(overrides: Partial<ScrapeFormState> = {}): ScrapeFormState {
  return {
    boards: ['aggregator'],
    query: '',
    location: 'Berlin, Germany',
    countryCode: 'DE',
    latitude: 52.52,
    longitude: 13.4,
    radiusKm: 0,
    amount: 25,
    dateFilter: '',
    companies: [],
    ...overrides,
  };
}

function renderFilters(onFormChange = vi.fn(), form = buildForm()) {
  render(
    <ScrapeFilters
      form={form}
      scraping={false}
      boardConnected={false}
      onFormChange={onFormChange}
      onGeocode={() => Promise.resolve([])}
    />
  );
  return onFormChange;
}

describe('ScrapeFilters — location', () => {
  it('clears the stale picked countryCode/lat/lon when the location is typed over', () => {
    const onFormChange = renderFilters();

    fireEvent.change(screen.getByTestId('location-input'), { target: { value: 'Amsterdam' } });

    expect(onFormChange).toHaveBeenCalledWith({
      location: 'Amsterdam',
      countryCode: undefined,
      latitude: undefined,
      longitude: undefined,
    });
  });

  it('keeps the structured payload when a suggestion is PICKED', () => {
    const onFormChange = renderFilters(
      vi.fn(),
      buildForm({ location: '', countryCode: undefined })
    );

    fireEvent.click(screen.getByText('pick'));

    // Assert on the LAST call, not "any call": the pick fires the clearing
    // onChange first, so a bare `toHaveBeenCalledWith` would still pass if the
    // clear ran second and threw the picked payload away.
    expect(onFormChange.mock.calls.at(-1)?.[0]).toEqual({
      location: 'Berlin, Germany',
      countryCode: 'DE',
      latitude: 52.52,
      longitude: 13.4,
    });
  });
});
