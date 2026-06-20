/**
 * ScrapeForm — catalog-driven picker tests.
 *
 * Covers:
 *   - Shows a skeleton while the catalog is loading
 *   - Renders one toggle button per listed board (glassdoor filtered out)
 *   - Renders no board buttons when the catalog returns []
 *   - glassdoor is NOT rendered (listed=false)
 *   - listed boards ARE rendered (greenhouse, linkedin, indeed)
 *   - Board order matches catalog (registry) order
 *   - Selected boards shown with aria-pressed=true
 *   - Select all / Clear controls present
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
  useBoardStatuses: () => ({ results: [], anyConnected: false }),
}));

// Stub ScrapeFilters — deep component; not under test here
vi.mock('./ScrapeFilters', () => ({
  ScrapeFilters: () => <div data-testid="scrape-filters" />,
}));

// Stub BoardConnectChip — not under test here
vi.mock('./BoardConnectChip', () => ({
  BoardConnectChip: ({ board }: { board: string }) => <span data-testid={`chip-${board}`} />,
}));

// i18n: identity t() so we assert on keys
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

// Stub multi-select key handler (returns a noop handler; no DOM layout needed)
vi.mock('@/hooks/use-roving-tabindex', () => ({
  makeMultiSelectKeyHandler: () => () => {},
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
  boards: ['greenhouse'],
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

describe('ScrapeForm — catalog loading state', () => {
  it('shows a skeleton while the catalog is loading', () => {
    renderForm({ catalogLoading: true, catalog: undefined });
    // Board group should not be present while loading
    expect(screen.queryByRole('group')).not.toBeInTheDocument();
  });
});

describe('ScrapeForm — catalog-driven board picker', () => {
  it('renders a toggle button for each listed board', () => {
    renderForm();
    // Each board button has role="button" and aria-pressed
    const buttons = screen
      .getAllByRole('button', { hidden: false })
      .filter((el) => el.getAttribute('aria-pressed') !== null);
    // 3 listed boards (greenhouse, linkedin, indeed); glassdoor is filtered out
    expect(buttons).toHaveLength(3);
  });

  it('does NOT render a button for glassdoor (listed=false)', () => {
    renderForm();
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
    const buttons = screen
      .getAllByRole('button', { hidden: false })
      .filter((el) => el.getAttribute('aria-pressed') !== null);
    const labels = buttons.map((r) => r.textContent ?? '');
    expect(labels[0]).toBe('jobs.boards.greenhouse');
    expect(labels[1]).toBe('jobs.boards.linkedin');
    expect(labels[2]).toBe('jobs.boards.indeed');
  });

  it('marks selected boards with aria-pressed=true', () => {
    renderForm();
    const greenhouse = screen.getByText('jobs.boards.greenhouse').closest('button');
    expect(greenhouse).toHaveAttribute('aria-pressed', 'true');
    const linkedin = screen.getByText('jobs.boards.linkedin').closest('button');
    expect(linkedin).toHaveAttribute('aria-pressed', 'false');
  });

  it('renders select-all and clear controls', () => {
    renderForm();
    expect(screen.getByText('jobs.selectAll')).toBeInTheDocument();
    expect(screen.getByText('jobs.clearBoards')).toBeInTheDocument();
  });
});

describe('ScrapeForm — empty catalog fallback', () => {
  it('renders no board toggle buttons when the catalog is empty', () => {
    renderForm({ catalog: [] });
    const buttons = screen
      .queryAllByRole('button', { hidden: false })
      .filter((el) => el.getAttribute('aria-pressed') !== null);
    expect(buttons).toHaveLength(0);
  });

  it('renders the group container even when catalog is empty (safe fallback)', () => {
    renderForm({ catalog: [] });
    expect(screen.getByRole('group')).toBeInTheDocument();
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
        onToggle={NOOP}
        onFormChange={NOOP}
        onStart={NOOP}
        onCancel={NOOP}
        onGeocode={async () => []}
      />
    );
    // AnimatePresence renders nothing when show=false
    expect(container.firstChild).toBeNull();
  });
});
