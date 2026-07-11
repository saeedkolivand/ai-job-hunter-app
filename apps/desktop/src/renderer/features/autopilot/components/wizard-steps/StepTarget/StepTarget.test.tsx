/**
 * StepTarget — countryCode wiring test (Fix A)
 *
 * Goal: assert that the LocationInput's onSelectSuggestion callback correctly
 * writes countryCode into the RHF form.
 *
 * Strategy:
 * - Mock @ajh/ui's LocationInput with a minimal test double that exposes a
 *   button which fires onSelectSuggestion with a fixed suggestion object.
 *   This is necessary because the real LocationInput uses a portal + async
 *   geocoding fetch that is impractical to drive from jsdom.
 * - Stub all heavy dependencies (boards catalog, AppClient, provider keys)
 *   with the lightest possible fakes so the component can render.
 * - Use the same RHF FormProvider wrapper pattern as StepSchedule.test.tsx.
 * - Assert the single thing that matters: after a suggestion pick, the RHF
 *   form's countryCode value equals the suggestion's countryCode.
 */

import { FormProvider, useForm, useFormContext } from 'react-hook-form';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';
import type * as AjhUi from '@ajh/ui';

import type { WizardState } from '@/features/autopilot/types';

import { StepTarget } from './index';

// ── Module stubs ──────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: 'en' } }),
}));

// Replace LocationInput with a test double backed by a vi.fn() so individual
// tests can override the emitted suggestion via mockImplementation.
// All other @ajh/ui exports pass through so the component renders normally.
type LocationInputStubProps = {
  onChange?: (v: string) => void;
  onSelectSuggestion?: (s: { display: string; countryCode?: string | null }) => void;
};

function defaultLocationInputImpl({ onChange, onSelectSuggestion }: LocationInputStubProps) {
  const pick = () => onSelectSuggestion?.({ display: 'London, UK', countryCode: 'gb' });
  const editManually = () => onChange?.('Lon');
  return (
    <>
      <div
        role="button"
        tabIndex={0}
        data-testid="location-input-stub"
        onClick={pick}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') pick();
        }}
      >
        pick-location
      </div>
      <div
        role="button"
        tabIndex={0}
        data-testid="location-input-manual-edit"
        onClick={editManually}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') editManually();
        }}
      >
        edit-location
      </div>
    </>
  );
}

const locationInputImpl = vi.fn(defaultLocationInputImpl);

vi.mock('@ajh/ui', async (importOriginal) => {
  const real = await importOriginal<typeof AjhUi>();
  return {
    ...real,
    LocationInput: (props: LocationInputStubProps) => locationInputImpl(props),
  };
});

// AppClient — only geocode.suggest is referenced (via onFetchSuggestions prop
// which our stub ignores, but the hook still calls useAppClient at render time).
vi.mock('@/providers/AppClientProvider', () => ({
  useAppClient: () => ({
    geocode: { suggest: () => Promise.resolve([]) },
  }),
}));

// Boards catalog — return a minimal listed board so the board selector renders.
vi.mock('@/services/use-boards', () => ({
  useBoardsCatalog: () => ({
    data: [{ id: 'aggregator', listed: true }],
    isLoading: false,
  }),
}));

// Provider-key queries — always "key absent" (safe default for the hint path).
vi.mock('@/services/use-ai-provider', () => ({
  useHasProviderKey: () => ({ data: { has: false } }),
}));

// Sub-components used inside StepTarget that bring in further heavy deps.
vi.mock('@/features/autopilot/components/wizard-steps/PrefilledBadge', () => ({
  PrefilledBadge: () => null,
}));

vi.mock('@/features/autopilot/components/wizard-steps/WizardField', () => ({
  WizardField: ({
    children,
  }: {
    children?: React.ReactNode;
    label?: string;
    htmlFor?: string;
    hint?: string;
    badge?: React.ReactNode;
    error?: string;
  }) => <>{children}</>,
}));

// ── Fixture helpers ───────────────────────────────────────────────────────────

function makeForm(overrides: Partial<WizardState> = {}): WizardState {
  return {
    name: 'Test run',
    boards: ['aggregator'],
    query: 'react developer',
    location: '',
    workType: 'any',
    amount: 50,
    dateFilter: '24h',
    minMatchScore: 50,
    keywords: '',
    excludeKeywords: '',
    resumeText: '',
    assistant: false,
    schedule: 'daily',
    scheduleHour: 9,
    scheduleMinute: 0,
    ...overrides,
  };
}

/** Exposes the live countryCode field as JSON for assertions. */
function Probe() {
  const { watch } = useFormContext<WizardState>();
  const countryCode = watch('countryCode');
  return <output data-testid={TEST_IDS.autopilot.probe}>{JSON.stringify({ countryCode })}</output>;
}

function renderStep(overrides: Partial<WizardState> = {}) {
  function Host() {
    const methods = useForm<WizardState>({ defaultValues: makeForm(overrides) });
    return (
      <FormProvider {...methods}>
        <StepTarget prefilled={{ location: false }} />
        <Probe />
      </FormProvider>
    );
  }
  return render(<Host />);
}

function readProbe(): { countryCode: string | undefined } {
  const text = screen.getByTestId(TEST_IDS.autopilot.probe).textContent ?? '{}';
  return JSON.parse(text) as { countryCode: string | undefined };
}

// ── Tests ─────────────────────────────────────────────────────────────────────

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
