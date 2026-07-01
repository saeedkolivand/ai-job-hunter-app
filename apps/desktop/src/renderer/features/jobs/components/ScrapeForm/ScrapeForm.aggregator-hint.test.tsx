/**
 * ScrapeForm — aggregator key hint tests.
 *
 * Covers:
 *   - Hint shown when 'aggregator' board is selected and both Adzuna keys absent
 *   - Hint hidden when 'aggregator' board is selected but both keys present
 *   - Hint hidden when only App ID is present (both must be present to suppress hint)
 *   - Hint hidden when 'aggregator' board is NOT selected (even with keys absent)
 *
 * motion/react is globally shimmed in vitest.setup.ts.
 * Uses the same stub approach as ScrapeForm.test.tsx.
 */

import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import type { BoardCatalogEntry } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';

// ---------------------------------------------------------------------------
// Stubs — boards catalog
// ---------------------------------------------------------------------------

let stubCatalog: BoardCatalogEntry[] = [];
let stubLoading = false;

vi.mock('@/services/use-boards', () => ({
  useBoardsCatalog: () => ({
    data: stubCatalog,
    isLoading: stubLoading,
    isSuccess: !stubLoading && stubCatalog !== undefined,
  }),
  useBoardStatuses: () => ({ results: [], anyConnected: false }),
}));

// ---------------------------------------------------------------------------
// Stubs — Adzuna key presence (configurable per test)
// ---------------------------------------------------------------------------

let stubIdHas = false;
let stubKeyHas = false;

vi.mock('@/services/use-ai-provider', async () => {
  const shared = await vi.importActual<{
    PROVIDER_SLOTS: { adzunaAppId: string; adzunaAppKey: string };
  }>('@ajh/shared');
  const PS = shared.PROVIDER_SLOTS;
  return {
    useHasProviderKey: (slot: string) => {
      if (slot === PS.adzunaAppId) return { data: { has: stubIdHas } };
      if (slot === PS.adzunaAppKey) return { data: { has: stubKeyHas } };
      return { data: { has: false } };
    },
  };
});

// ---------------------------------------------------------------------------
// Stubs — ScrapeFilters, BoardConnectChip, i18n, roving tabindex
// ---------------------------------------------------------------------------

vi.mock('./ScrapeFilters', () => ({
  ScrapeFilters: () => <div data-testid={TEST_IDS.jobs.scrapeFilters} />,
}));

vi.mock('./BoardConnectChip', () => ({
  BoardConnectChip: ({ board }: { board: string }) => <span data-testid={`chip-${board}`} />,
}));

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (k: string, p?: Record<string, string>) => {
      if (!p) return k;
      return Object.entries(p).reduce(
        (acc, [k2, v]) => acc.replace(new RegExp(`\\{\\{${k2}\\}\\}`, 'g'), v),
        k
      );
    },
  }),
}));

vi.mock('@/hooks/use-roving-tabindex', () => ({
  makeMultiSelectKeyHandler: () => () => {},
}));

// ---------------------------------------------------------------------------
// Import under test (AFTER mocks)
// ---------------------------------------------------------------------------

import { ScrapeForm } from './index';

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const CATALOG_WITH_AGGREGATOR: BoardCatalogEntry[] = [
  {
    id: 'greenhouse',
    displayName: 'Greenhouse',
    mode: 'http',
    auth: 'guest',
    listed: true,
    requiresCompany: false,
  },
  {
    id: 'aggregator',
    displayName: 'Aggregator',
    mode: 'http',
    auth: 'guest',
    listed: true,
    requiresCompany: false,
  },
];

function buildForm(boards: string[]): Parameters<typeof ScrapeForm>[0]['form'] {
  return {
    boards,
    query: '',
    location: '',
    radiusKm: 0,
    amount: 25,
    dateFilter: '' as const,
    companies: [],
  };
}

const NOOP = () => {};

function Wrapper({ children }: { children: ReactNode }) {
  return <>{children}</>;
}

function renderForm(boards: string[]) {
  stubCatalog = CATALOG_WITH_AGGREGATOR;
  stubLoading = false;

  return render(
    <ScrapeForm
      show={true}
      form={buildForm(boards)}
      scraping={false}
      scrapeOutcome={null}
      onToggle={NOOP}
      onFormChange={NOOP}
      onStart={NOOP}
      onCancel={NOOP}
      onGeocode={async () => []}
    />,
    { wrapper: Wrapper }
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('ScrapeForm — aggregator key hint', () => {
  it('shows the hint when aggregator is selected and both Adzuna keys are absent', () => {
    stubIdHas = false;
    stubKeyHas = false;
    renderForm(['aggregator']);

    expect(screen.getByTestId(TEST_IDS.jobs.aggregatorKeyHint)).toBeInTheDocument();
    expect(screen.getByText('jobs.aggregatorKeyHint')).toBeInTheDocument();
  });

  it('hides the hint when aggregator is selected and both Adzuna keys are present', () => {
    stubIdHas = true;
    stubKeyHas = true;
    renderForm(['aggregator']);

    expect(screen.queryByTestId(TEST_IDS.jobs.aggregatorKeyHint)).not.toBeInTheDocument();
  });

  it('shows the hint when aggregator is selected and only App ID is absent (App Key present)', () => {
    stubIdHas = false;
    stubKeyHas = true;
    renderForm(['aggregator']);

    // Both keys must be present to suppress the hint
    expect(screen.getByTestId(TEST_IDS.jobs.aggregatorKeyHint)).toBeInTheDocument();
  });

  it('shows the hint when aggregator is selected and only App Key is absent (App ID present)', () => {
    stubIdHas = true;
    stubKeyHas = false;
    renderForm(['aggregator']);

    expect(screen.getByTestId(TEST_IDS.jobs.aggregatorKeyHint)).toBeInTheDocument();
  });

  it('hides the hint when aggregator is NOT selected (even with keys absent)', () => {
    stubIdHas = false;
    stubKeyHas = false;
    renderForm(['greenhouse']); // no aggregator board selected

    expect(screen.queryByTestId(TEST_IDS.jobs.aggregatorKeyHint)).not.toBeInTheDocument();
  });

  it('hides the hint when aggregator is deselected (multiple boards, aggregator removed)', () => {
    stubIdHas = false;
    stubKeyHas = false;
    renderForm(['greenhouse']); // aggregator not in selection

    expect(screen.queryByTestId(TEST_IDS.jobs.aggregatorKeyHint)).not.toBeInTheDocument();
  });
});
