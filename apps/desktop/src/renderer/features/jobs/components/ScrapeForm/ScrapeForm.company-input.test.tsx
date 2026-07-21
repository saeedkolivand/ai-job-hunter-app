/**
 * ScrapeForm — showCompanyInput derivation + the stale-value clear.
 *
 * The company field is now the ADR-030 slug typeahead (`CompanySlugField`),
 * which owns its own behavior (covered in CompanySlugField.test.tsx). Here we
 * only assert ScrapeForm's own logic: WHEN the field is shown (driven purely by
 * the selected boards' `requiresCompany` flag) and that a stale `companies`
 * array is cleared when the field disappears. CompanySlugField is stubbed so
 * these stay focused and provider-free.
 *
 * Strategy: render ScrapeForm with a mocked useBoardsCatalog — same pattern as
 * the sibling ScrapeForm.test.tsx. onFormChange is a vi.fn() so we inspect it.
 */
import { act, type ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import type { BoardCatalogEntry } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';

// ---------------------------------------------------------------------------
// Module-level stubs — reset before each test to prevent state leaks.
// ---------------------------------------------------------------------------

let stubCatalog: BoardCatalogEntry[] | undefined = undefined;
let stubLoading = false;

beforeEach(() => {
  stubCatalog = undefined;
  stubLoading = false;
});

vi.mock('@/services/use-boards', () => ({
  useBoardsCatalog: () => ({
    data: stubCatalog,
    isLoading: stubLoading,
    isSuccess: !stubLoading && stubCatalog !== undefined,
  }),
  useBoardStatuses: () => ({ results: [], anyConnected: false }),
}));

// Stub out Adzuna key presence — not under test in this file.
vi.mock('@/services/use-ai-provider', () => ({
  useHasProviderKey: () => ({ data: { has: false } }),
}));

vi.mock('./ScrapeFilters', () => ({
  ScrapeFilters: () => <div data-testid={TEST_IDS.jobs.scrapeFilters} />,
}));

// Stub the slug typeahead — its behavior is covered in CompanySlugField.test.tsx.
// It renders an element carrying the typeahead test id so visibility asserts work.
vi.mock('./CompanySlugField', () => ({
  CompanySlugField: () => <div data-testid={TEST_IDS.jobs.companyTypeahead} />,
}));

vi.mock('./BoardConnectChip', () => ({
  BoardConnectChip: ({ board }: { board: string }) => <span data-testid={`chip-${board}`} />,
}));

vi.mock('@/hooks/use-roving-tabindex', () => ({
  makeMultiSelectKeyHandler: () => () => {},
}));

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (k: string, p?: Record<string, string | number>) => {
      if (!p) return k;
      return Object.entries(p).reduce(
        (acc, [key, val]) => acc.replace(new RegExp(`\\{\\{${key}\\}\\}`, 'g'), String(val)),
        k
      );
    },
  }),
}));

// Import under test AFTER mocks.
import { ScrapeForm } from './index';

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/** A board that does NOT require a company slug. */
const BOARD_PLAIN: BoardCatalogEntry = {
  id: 'linkedin',
  displayName: 'LinkedIn',
  mode: 'http',
  auth: 'optional',
  listed: true,
  requiresCompany: false,
};

/** A board that DOES require a company slug (novel id — no hardcoding). */
const BOARD_ATS: BoardCatalogEntry = {
  id: 'novel-ats-board',
  displayName: 'Novel ATS',
  mode: 'http',
  auth: 'guest',
  listed: true,
  requiresCompany: true,
};

function buildForm(
  boards: string[],
  companies: string[] = []
): Parameters<typeof ScrapeForm>[0]['form'] {
  return {
    boards,
    query: 'engineer',
    location: '',
    radiusKm: 0,
    amount: 25,
    dateFilter: '' as const,
    companies,
  };
}

function Wrapper({ children }: { children: ReactNode }) {
  return <>{children}</>;
}

const DEFAULT_CATALOG: BoardCatalogEntry[] = [BOARD_PLAIN, BOARD_ATS];

function renderForm(
  boards: string[],
  companies: string[] = [],
  onFormChange: Parameters<typeof ScrapeForm>[0]['onFormChange'] = vi.fn(),
  catalog: BoardCatalogEntry[] = DEFAULT_CATALOG,
  loading = false
) {
  stubCatalog = loading ? undefined : catalog;
  stubLoading = loading;

  return render(
    <ScrapeForm
      show={true}
      form={buildForm(boards, companies)}
      scraping={false}
      scrapeOutcome={null}
      onToggle={vi.fn()}
      onFormChange={onFormChange}
      onStart={vi.fn()}
      onCancel={vi.fn()}
      onGeocode={async () => []}
    />,
    { wrapper: Wrapper }
  );
}

/** Returns the slug typeahead, or null if the field is not shown. */
function getCompanyField(): HTMLElement | null {
  return screen.queryByTestId(TEST_IDS.jobs.companyTypeahead);
}

// ---------------------------------------------------------------------------
// showCompanyInput — field visibility driven by requiresCompany flag
// ---------------------------------------------------------------------------

describe('ScrapeForm — showCompanyInput derivation', () => {
  it('shows the company field when the selected board has requiresCompany:true', () => {
    renderForm([BOARD_ATS.id]);
    expect(getCompanyField()).not.toBeNull();
  });

  it('hides the company field when only non-requiresCompany boards are selected', () => {
    renderForm([BOARD_PLAIN.id]);
    expect(getCompanyField()).toBeNull();
  });

  it('shows the field when a requiresCompany board is among multiple selected boards', () => {
    renderForm([BOARD_PLAIN.id, BOARD_ATS.id]);
    expect(getCompanyField()).not.toBeNull();
  });

  it('hides the field when the catalog is empty', () => {
    renderForm([BOARD_ATS.id], [], vi.fn(), []);
    expect(getCompanyField()).toBeNull();
  });

  it('hides the field while the catalog is loading (no crash)', () => {
    renderForm([BOARD_ATS.id], [], vi.fn(), undefined, true);
    expect(getCompanyField()).toBeNull();
  });

  it('does NOT hardcode board ids — a novel requiresCompany board still shows the field', () => {
    // BOARD_ATS has id 'novel-ats-board', which doesn't exist in the real registry.
    // The component must derive visibility from the flag alone.
    renderForm([BOARD_ATS.id]);
    expect(getCompanyField()).not.toBeNull();
  });

  it('a requiresCompany:false board never shows the field even when selected', () => {
    const customCatalog: BoardCatalogEntry[] = [{ ...BOARD_PLAIN, requiresCompany: false }];
    renderForm([BOARD_PLAIN.id], [], vi.fn(), customCatalog);
    expect(getCompanyField()).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Stale-value guard — when the requiresCompany board is deselected, the field
// disappears and companies is cleared so no stale value reaches the scrape.
// ---------------------------------------------------------------------------

describe('ScrapeForm — companies cleared after deselecting the requiresCompany board', () => {
  it('company field is absent when only non-requiresCompany boards are selected', () => {
    renderForm([BOARD_PLAIN.id], ['stripe']);
    expect(getCompanyField()).toBeNull();
    expect(screen.queryByText('jobs.companies.label')).toBeNull();
  });

  it('clears companies via onFormChange when showCompanyInput transitions to false', async () => {
    stubCatalog = DEFAULT_CATALOG;
    stubLoading = false;
    const onFormChange = vi.fn();
    const { rerender } = render(
      <ScrapeForm
        show={true}
        form={buildForm([BOARD_ATS.id], ['stripe'])}
        scraping={false}
        scrapeOutcome={null}
        onToggle={vi.fn()}
        onFormChange={onFormChange}
        onStart={vi.fn()}
        onCancel={vi.fn()}
        onGeocode={async () => []}
      />,
      { wrapper: Wrapper }
    );

    onFormChange.mockClear();

    // Deselect the ATS board → showCompanyInput becomes false. Wrap in act() so
    // the clearing effect flushes synchronously.
    await act(async () => {
      rerender(
        <ScrapeForm
          show={true}
          form={buildForm([BOARD_PLAIN.id], ['stripe'])}
          scraping={false}
          scrapeOutcome={null}
          onToggle={vi.fn()}
          onFormChange={onFormChange}
          onStart={vi.fn()}
          onCancel={vi.fn()}
          onGeocode={async () => []}
        />
      );
    });

    const clearCall = onFormChange.mock.calls.find(
      (c) =>
        Array.isArray((c[0] as { companies?: unknown })?.companies) &&
        (c[0] as { companies: unknown[] }).companies.length === 0
    );
    expect(clearCall).toBeDefined();
  });
});
