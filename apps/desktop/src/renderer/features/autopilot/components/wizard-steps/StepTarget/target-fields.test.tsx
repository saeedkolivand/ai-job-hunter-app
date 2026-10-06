/**
 * StepTarget — location + country, work type, filter-note / seeded-companies / watched-companies integration
 */

import { afterEach, describe, expect, it } from 'vitest';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

import {
  boardsCatalogImpl,
  DEFAULT_CATALOG,
  defaultLocationInputImpl,
  locationInputImpl,
  type LocationInputStubProps,
  readProbe,
  renderStep,
} from './test-support';

describe('StepTarget — countryCode wiring (Fix A)', () => {
  afterEach(() => {
    // Restore the default 'gb' implementation so tests are independent.
    locationInputImpl.mockImplementation(defaultLocationInputImpl);
  });

  it('writes countryCode into the form when a location suggestion is picked', async () => {
    const user = userEvent.setup();
    renderStep({ countryCode: undefined });

    // The stub LocationInput renders a button; clicking it fires onSelectSuggestion
    // with { display: 'London, UK', countryCode: 'gb' }.
    await user.click(screen.getByTestId('location-input-stub'));

    expect(readProbe().countryCode).toBe('gb');
  });

  it('shows the derived "Country" line after a suggestion pick, and hides it again on manual edit', async () => {
    const user = userEvent.setup();
    renderStep({ countryCode: undefined });

    expect(screen.queryByText('autopilot.wizard.target.countryResolved')).toBeNull();

    await user.click(screen.getByTestId('location-input-stub'));
    expect(screen.getByText('autopilot.wizard.target.countryResolved')).toBeInTheDocument();

    // Manually editing the location clears countryCode (index.tsx's onChange
    // handler) — the derived-country line must disappear with it.
    await user.click(screen.getByTestId('location-input-manual-edit'));
    expect(screen.queryByText('autopilot.wizard.target.countryResolved')).toBeNull();
  });

  it('renders a warning Alert for the aggregator key hint when aggregator is selected and keys are absent', () => {
    // The mock stubs already have: board=['aggregator'], useHasProviderKey → has:false.
    // So showAggregatorKeyHint=true and the Alert should appear.
    renderStep({ boards: ['aggregator'] });
    const alert = screen.getByRole('alert');
    expect(alert).toBeInTheDocument();
    expect(alert).toHaveTextContent('jobs.aggregatorKeyHint');
  });

  it('coerces null countryCode to undefined via the ?? undefined guard', async () => {
    // LocationInput.Suggestion allows countryCode: string | null | undefined.
    // The production handler does `s.countryCode ?? undefined` — null must not
    // bleed through; the form value must be undefined, not null.
    //
    // Use mockImplementation (not Once) — re-renders from the board-normalization
    // useEffect would consume a mockImplementationOnce before the click fires.
    locationInputImpl.mockImplementation(({ onSelectSuggestion }: LocationInputStubProps) => {
      const handler = () => onSelectSuggestion?.({ display: 'Berlin', countryCode: null });
      return (
        <div
          role="button"
          tabIndex={0}
          data-testid="location-input-stub"
          onClick={handler}
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') handler();
          }}
        >
          pick-location
        </div>
      );
    });

    const user = userEvent.setup();
    renderStep({ countryCode: undefined });

    await user.click(screen.getByTestId('location-input-stub'));

    expect(readProbe().countryCode).toBeUndefined();
  });
});

// ── LocationFilterNote integration (PR F) ───────────────────────────────────
// Real component, not stubbed here, so a wrong prop name at the StepTarget
// call site would fail this render instead of silently compiling and passing
// every other test in the file.

describe('StepTarget — location filter note (PR F integration)', () => {
  it('shows the note when a location is set and the selected board does not support it', () => {
    // catalog stub: aggregator, listed, no `supportsLocation` — falsy, non-supporting.
    renderStep({ boards: ['aggregator'], location: 'Berlin' });
    expect(screen.getByRole('note')).toBeInTheDocument();
  });

  it('hides the note when no location is set (default empty location)', () => {
    renderStep({ boards: ['aggregator'], location: '' });
    expect(screen.queryByRole('note')).not.toBeInTheDocument();
  });
});

// ── Work-type multi-select (restored control, deleted as a disabled stub in
// #614, rebuilt as a real enabled multi-select) ─────────────────────────────

describe('StepTarget — work type multi-select', () => {
  it('renders all three options unselected by default and toggles aria-pressed on click', async () => {
    const user = userEvent.setup();
    renderStep({ workTypes: [] });

    const remote = screen.getByRole('button', { name: 'jobs.workType.remote' });
    const hybrid = screen.getByRole('button', { name: 'jobs.workType.hybrid' });
    const onSite = screen.getByRole('button', { name: 'jobs.workType.on-site' });
    for (const btn of [remote, hybrid, onSite]) {
      expect(btn).toHaveAttribute('aria-pressed', 'false');
    }

    await user.click(hybrid);
    expect(hybrid).toHaveAttribute('aria-pressed', 'true');
    expect(remote).toHaveAttribute('aria-pressed', 'false');
  });

  it('toggling twice returns to unselected (it is a set, not a radio)', async () => {
    const user = userEvent.setup();
    renderStep({ workTypes: [] });

    const remote = screen.getByRole('button', { name: 'jobs.workType.remote' });
    await user.click(remote);
    expect(remote).toHaveAttribute('aria-pressed', 'true');
    await user.click(remote);
    expect(remote).toHaveAttribute('aria-pressed', 'false');
  });

  it('seeds selection from a persisted workTypes array', () => {
    renderStep({ workTypes: ['hybrid', 'on-site'] });

    expect(screen.getByRole('button', { name: 'jobs.workType.hybrid' })).toHaveAttribute(
      'aria-pressed',
      'true'
    );
    expect(screen.getByRole('button', { name: 'jobs.workType.on-site' })).toHaveAttribute(
      'aria-pressed',
      'true'
    );
    expect(screen.getByRole('button', { name: 'jobs.workType.remote' })).toHaveAttribute(
      'aria-pressed',
      'false'
    );
  });

  it('gives the control group an accessible name', () => {
    renderStep({ workTypes: [] });
    expect(
      screen.getByRole('group', { name: 'autopilot.wizard.target.workType' })
    ).toBeInTheDocument();
  });

  it('shows visible "any" microcopy next to the label when the set is empty', () => {
    renderStep({ workTypes: [] });
    expect(screen.getByText('jobs.workType.any')).toBeInTheDocument();
  });

  it('hides the "any" microcopy once at least one work type is picked', () => {
    renderStep({ workTypes: ['remote'] });
    expect(screen.queryByText('jobs.workType.any')).toBeNull();
  });

  it('every option is an independent tab stop — no roving tabindex for a 3-item set', () => {
    renderStep({ workTypes: [] });
    const group = screen.getByRole('group', { name: 'autopilot.wizard.target.workType' });
    // Roving tabindex would set explicit tabIndex={-1}/{0} attributes; a plain
    // tab stop leaves the native button default (no `tabindex` attribute at
    // all), so every option in a 3-item set is independently Tab-reachable —
    // matching ScrapeFilters' manual-search control (same keyboard model).
    for (const btn of within(group).getAllByRole('button')) {
      expect(btn).not.toHaveAttribute('tabindex');
    }
  });
});

// ── WorkTypeFilterNote integration ───────────────────────────────────────────
// Real component, not stubbed, so a wrong prop name at the StepTarget call
// site would fail this render — mirrors the LocationFilterNote integration
// block above.

describe('StepTarget — work type filter note integration', () => {
  it('shows the note when a work type is picked and the selected board does not support it', async () => {
    const user = userEvent.setup();
    // catalog stub: aggregator, listed, no `supportsWorkType` — falsy, non-supporting.
    renderStep({ boards: ['aggregator'], workTypes: [] });

    await user.click(screen.getByRole('button', { name: 'jobs.workType.remote' }));

    // Two `role="note"` elements can coexist (location + work type); scope by content.
    const notes = screen.getAllByRole('note');
    expect(notes.some((n) => n.textContent?.includes('jobs.workType.filterSummary'))).toBe(true);
  });

  it('hides the note when no work type is picked (default empty selection)', () => {
    renderStep({ boards: ['aggregator'], workTypes: [] });
    const notes = screen.queryAllByRole('note');
    expect(notes.some((n) => n.textContent?.includes('jobs.workType.filterSummary'))).toBe(false);
  });
});

// ── SeededCompaniesNote integration (#621) ──────────────────────────────────
// Real component, not stubbed here, so a wrong prop name at the StepTarget
// call site would fail this render instead of silently compiling and passing
// every other test in the file. Uses the overridable `boardsCatalogImpl` mock
// so each test can supply its own `seededCompanies` catalog fixture.

describe('StepTarget — seeded companies disclosure (#621 integration)', () => {
  afterEach(() => {
    boardsCatalogImpl.mockReturnValue({ data: DEFAULT_CATALOG, isLoading: false });
  });

  it('shows the disclosure for a selected board with seededCompanies, truncated to 5 names + more', () => {
    boardsCatalogImpl.mockReturnValue({
      data: [
        {
          id: 'greenhouse',
          listed: true,
          seededCompanies: ['Stripe', 'Airbnb', 'OpenAI', 'Bosch', 'N26', 'Lyft'],
        },
      ],
      isLoading: false,
    });
    renderStep({ boards: ['greenhouse'] });

    const note = screen.getByRole('note');
    expect(note.textContent).toContain('Stripe');
    expect(note.textContent).toContain('Airbnb');
    expect(note.textContent).toContain('OpenAI');
    expect(note.textContent).toContain('Bosch');
    expect(note.textContent).toContain('N26');
    // 6th name truncated away; the pluralized "more" key fired instead (real
    // interpolated count covered by SeededCompaniesNote.i18n.test.ts).
    expect(note.textContent).not.toContain('Lyft');
    expect(note.textContent).toContain('autopilot.wizard.target.seededCompanies.more');
  });

  it('shows no disclosure for a selected board with no seededCompanies', () => {
    boardsCatalogImpl.mockReturnValue({
      data: [{ id: 'greenhouse', listed: true }],
      isLoading: false,
    });
    renderStep({ boards: ['greenhouse'] });

    expect(screen.queryByRole('note')).not.toBeInTheDocument();
  });
});

// ── WatchedCompaniesField integration (ADR-030 §e) ──────────────────────────
// The REAL WatchedCompaniesField is rendered (discovery service hooks stubbed),
// so its insertion into the target step is actually asserted — a wrong/removed
// call site fails here instead of passing silently.

describe('StepTarget — watched-companies target (ADR-030 integration)', () => {
  it('renders the watched-companies toggle in the target step', () => {
    renderStep({ boards: ['aggregator'] });
    expect(screen.getByTestId(TEST_IDS.autopilot.watchedCompaniesToggle)).toBeInTheDocument();
  });
});
