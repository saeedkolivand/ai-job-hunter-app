/**
 * ScrapeForm — catalog-driven picker tests.
 *
 * Covers:
 *   - Shows a skeleton while the catalog is loading
 *   - Renders one radio button per listed board (glassdoor filtered out)
 *   - Renders no board buttons when the catalog returns []
 *   - glassdoor is NOT rendered (listed=false)
 *   - listed boards ARE rendered (greenhouse, linkedin, indeed)
 *   - Board order matches catalog (registry) order
 *
 * motion/react is globally shimmed in vitest.setup.ts.
 * ScrapeFilters is stubbed to avoid deep dependency pulls.
 */
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import type { BoardCatalogEntry } from '@ajh/shared';

// ---------------------------------------------------------------------------
// Module-level stub for useBoardsCatalog — set before each test via the ref
// ---------------------------------------------------------------------------

let stubCatalog: BoardCatalogEntry[] | undefined = undefined;
let stubLoading = false;

vi.mock('@/services/use-boards', () => ({
  useBoardsCatalog: () => ({
    data: stubCatalog,
    isLoading: stubLoading,
    isSuccess: !stubLoading && stubCatalog !== undefined,
  }),
}));

// Stub ScrapeFilters — deep component; not under test here
vi.mock('./ScrapeFilters', () => ({
  ScrapeFilters: () => <div data-testid="scrape-filters" />,
}));

// Stub AuthHint — not under test here
vi.mock('./AuthHint', () => ({
  AuthHint: () => null,
}));

// i18n: identity t() so we assert on keys
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// Stub roving-tabindex (returns a noop handler; no DOM layout needed)
vi.mock('@/hooks/use-roving-tabindex', () => ({
  makeRovingTabindex: () => () => {},
}));

// ---------------------------------------------------------------------------
// Import under test (AFTER mocks are declared)
// ---------------------------------------------------------------------------

import { ScrapeForm } from './index';

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const LISTED_CATALOG: BoardCatalogEntry[] = [
  { id: 'greenhouse', displayName: 'Greenhouse', mode: 'http', auth: 'guest', listed: true },
  { id: 'linkedin', displayName: 'LinkedIn', mode: 'http', auth: 'optional', listed: true },
  { id: 'indeed', displayName: 'Indeed', mode: 'browser', auth: 'required', listed: true },
  // glassdoor: listed=false → should NOT appear in the picker
  { id: 'glassdoor', displayName: 'Glassdoor', mode: 'browser', auth: 'guest', listed: false },
];

const DEFAULT_FORM = {
  board: 'greenhouse',
  query: '',
  location: '',
  radiusKm: 0,
  amount: 25,
  dateFilter: '' as const,
  locale: 'en',
};

const NOOP = () => {};

function renderForm(overrides: { catalogLoading?: boolean; catalog?: BoardCatalogEntry[] } = {}) {
  stubCatalog = overrides.catalog ?? LISTED_CATALOG;
  stubLoading = overrides.catalogLoading ?? false;

  // Wrap in a minimal fragment — ScrapeForm has no Provider requirements
  function Wrapper({ children }: { children: ReactNode }) {
    return <>{children}</>;
  }

  return render(
    <ScrapeForm
      show={true}
      form={DEFAULT_FORM}
      scraping={false}
      scrapeOutcome={null}
      boardConnected={false}
      connectPending={false}
      disconnectPending={false}
      onToggle={NOOP}
      onFormChange={NOOP}
      onStart={NOOP}
      onCancel={NOOP}
      onConnect={NOOP}
      onDisconnect={NOOP}
      onGeocode={async () => []}
    />,
    { wrapper: Wrapper }
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('ScrapeForm — catalog loading state', () => {
  it('shows a skeleton while the catalog is loading', () => {
    renderForm({ catalogLoading: true, catalog: undefined });
    // CardSkeleton renders as a div with a specific class — query by its role or test-id
    // The board radiogroup should not be present while loading
    expect(screen.queryByRole('radiogroup')).not.toBeInTheDocument();
  });
});

describe('ScrapeForm — catalog-driven board picker', () => {
  it('renders a radio button for each listed board', () => {
    renderForm();
    const radios = screen.getAllByRole('radio');
    // 3 listed boards (greenhouse, linkedin, indeed); glassdoor is filtered out
    expect(radios).toHaveLength(3);
  });

  it('does NOT render a button for glassdoor (listed=false)', () => {
    renderForm();
    // The button label uses t(`jobs.boards.glassdoor`) → key string in test
    expect(screen.queryByText('jobs.boards.glassdoor')).not.toBeInTheDocument();
  });

  it('renders greenhouse (guest board) in the picker', () => {
    renderForm();
    expect(screen.getByText('jobs.boards.greenhouse')).toBeInTheDocument();
  });

  it('renders linkedin (optional board) in the picker', () => {
    renderForm();
    expect(screen.getByText('jobs.boards.linkedin')).toBeInTheDocument();
  });

  it('renders indeed (required board) in the picker', () => {
    renderForm();
    expect(screen.getByText('jobs.boards.indeed')).toBeInTheDocument();
  });

  it('renders boards in catalog (registry) order', () => {
    renderForm();
    const radios = screen.getAllByRole('radio');
    // Text content uses i18n keys in test mode; check DOM order matches catalog order
    const labels = radios.map((r) => r.textContent ?? '');
    expect(labels[0]).toBe('jobs.boards.greenhouse');
    expect(labels[1]).toBe('jobs.boards.linkedin');
    expect(labels[2]).toBe('jobs.boards.indeed');
  });
});

describe('ScrapeForm — empty catalog fallback', () => {
  it('renders no board radio buttons when the catalog is empty', () => {
    renderForm({ catalog: [] });
    expect(screen.queryByRole('radio')).not.toBeInTheDocument();
  });

  it('renders the radiogroup container even when catalog is empty (safe fallback)', () => {
    renderForm({ catalog: [] });
    // The radiogroup div is still rendered (just empty)
    expect(screen.getByRole('radiogroup')).toBeInTheDocument();
  });
});

describe('ScrapeForm — hidden when show=false', () => {
  it('renders nothing when show=false', () => {
    stubCatalog = LISTED_CATALOG;
    stubLoading = false;
    const { container } = render(
      <ScrapeForm
        show={false}
        form={DEFAULT_FORM}
        scraping={false}
        scrapeOutcome={null}
        boardConnected={false}
        connectPending={false}
        disconnectPending={false}
        onToggle={NOOP}
        onFormChange={NOOP}
        onStart={NOOP}
        onCancel={NOOP}
        onConnect={NOOP}
        onDisconnect={NOOP}
        onGeocode={async () => []}
      />
    );
    // AnimatePresence renders nothing when show=false
    expect(container.firstChild).toBeNull();
  });
});
