/**
 * Shared mocks + fixtures for the StepTarget suites.
 *
 * Mock strategy: @ajh/ui's LocationInput is replaced with a test double that
 * exposes buttons firing onSelectSuggestion / onChange (the real one uses a
 * portal + async geocoding fetch that is impractical to drive from jsdom); all
 * heavy dependencies (boards catalog, AppClient, provider keys) are stubbed with
 * the lightest possible fakes. This module registers every `vi.mock` (hoisted
 * above its own imports), so a suite must reach the step through `renderStep`.
 */

import { FormProvider, useForm, useFormContext } from 'react-hook-form';
import { vi } from 'vitest';
import { zodResolver } from '@hookform/resolvers/zod';
import { render, screen } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';
import type * as AjhUi from '@ajh/ui';
import { NotificationProvider } from '@ajh/ui';

import { autopilotWizardSchema } from '@/features/autopilot/lib/schema';
import type { WizardState } from '@/features/autopilot/types';

import { StepTarget } from './index';

// ── Module stubs ──────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: 'en' } }),
}));

// Replace LocationInput with a test double backed by a vi.fn() so individual
// tests can override the emitted suggestion via mockImplementation.
// All other @ajh/ui exports pass through so the component renders normally.
export type LocationInputStubProps = {
  onChange?: (v: string) => void;
  onSelectSuggestion?: (s: { display: string; countryCode?: string | null }) => void;
};

export function defaultLocationInputImpl({ onChange, onSelectSuggestion }: LocationInputStubProps) {
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

export const locationInputImpl = vi.fn(defaultLocationInputImpl);

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
// Overridable per-test (mockReturnValue) for the seeded-companies disclosure tests
// — explicitly typed so `seededCompanies` is a valid (optional) fixture field.
type CatalogBoardFixture = { id: string; listed: boolean; seededCompanies?: string[] };

// Two listed boards so a non-aggregator / MIXED selection is expressible: the
// board-normalization effect drops any selected id missing from the catalog, so
// the page-budget tests below could not select anything else without this.
export const DEFAULT_CATALOG: CatalogBoardFixture[] = [
  { id: 'aggregator', listed: true },
  { id: 'greenhouse', listed: true },
];

export const boardsCatalogImpl = vi.fn((): { data: CatalogBoardFixture[]; isLoading: boolean } => ({
  data: DEFAULT_CATALOG,
  isLoading: false,
}));

vi.mock('@/services/use-boards', () => ({
  useBoardsCatalog: () => boardsCatalogImpl(),
}));

// Provider-key queries — always "key absent" (safe default for the hint path).
vi.mock('@/services/use-ai-provider', () => ({
  useHasProviderKey: () => ({ data: { has: false } }),
}));

// Sub-components used inside StepTarget that bring in further heavy deps.
vi.mock('@/features/autopilot/components/wizard-steps/PrefilledBadge', () => ({
  PrefilledBadge: () => null,
}));

// WizardField is NOT stubbed: it is a dependency-free presentational wrapper and
// it owns the label/`htmlFor` wiring, so stubbing it would hide a control that
// ships without an accessible name (see the page-budget field tests below).

// Render the REAL WatchedCompaniesField so its insertion into the target step is
// actually asserted — but stub the discovery SERVICE hooks so no React Query /
// AppClient wiring is needed (its own behavior is covered in
// WatchedCompaniesField.test.tsx). `useNotification` still needs a provider,
// added in renderStep.
vi.mock('@/services/use-discovery', () => ({
  useWatchedCompanies: () => ({ data: [] }),
  useSetStarred: () => ({ mutate: vi.fn(), isPending: false }),
}));

// ── Fixture helpers ───────────────────────────────────────────────────────────

function makeForm(overrides: Partial<WizardState> = {}): WizardState {
  return {
    name: 'Test run',
    boards: ['aggregator'],
    query: 'react developer',
    location: '',
    workTypes: [],
    pages: 2,
    dateFilter: '24h',
    watchedCompaniesOnly: false,
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

/**
 * Exposes the live countryCode + pages fields, plus resolver validity, as JSON.
 *
 * `isValid` is what actually gates the wizard's "Next"/"Create", so it is the only
 * honest way to assert "the user is not blocked". Subscribing to it also makes RHF
 * run a validation pass on mount, which is precisely the moment a persisted bad
 * value would strand the user on a disabled control.
 */
function Probe() {
  const { watch, formState } = useFormContext<WizardState>();
  const countryCode = watch('countryCode');
  const pages = watch('pages');
  return (
    <output data-testid={TEST_IDS.autopilot.probe}>
      {JSON.stringify({ countryCode, pages, isValid: formState.isValid })}
    </output>
  );
}

export function renderStep(overrides: Partial<WizardState> = {}) {
  function Host() {
    // Same resolver + mode as CreationWizard, so validation-driven UI (inline
    // error text, aria-invalid) is exercised here rather than assumed.
    const methods = useForm<WizardState>({
      defaultValues: makeForm(overrides),
      resolver: zodResolver(autopilotWizardSchema),
      mode: 'onChange',
    });
    return (
      <NotificationProvider>
        <FormProvider {...methods}>
          <StepTarget prefilled={{ location: false }} />
          <Probe />
        </FormProvider>
      </NotificationProvider>
    );
  }
  return render(<Host />);
}

export function readProbe(): { countryCode: string | undefined; pages: number; isValid: boolean } {
  const text = screen.getByTestId(TEST_IDS.autopilot.probe).textContent ?? '{}';
  return JSON.parse(text) as {
    countryCode: string | undefined;
    pages: number;
    isValid: boolean;
  };
}
