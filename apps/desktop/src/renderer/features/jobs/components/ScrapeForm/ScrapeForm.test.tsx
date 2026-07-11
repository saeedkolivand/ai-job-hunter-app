/**
 * ScrapeForm — catalog-driven picker tests.
 *
 * Covers:
 *   - Shows a skeleton while the catalog is loading
 *   - Renders one toggle button per listed board (remoteok filtered out)
 *   - Renders no board buttons when the catalog returns []
 *   - remoteok is NOT rendered (listed=false)
 *   - listed boards ARE rendered (greenhouse, linkedin, lever)
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
import { TEST_IDS } from '@ajh/test-ids';

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

// Stub out Adzuna key presence — not under test in this file.
vi.mock('@/services/use-ai-provider', () => ({
  useHasProviderKey: () => ({ data: { has: false } }),
}));

// Stub ScrapeFilters — deep component; not under test here
vi.mock('./ScrapeFilters', () => ({
  ScrapeFilters: () => <div data-testid={TEST_IDS.jobs.scrapeFilters} />,
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
  {
    id: 'greenhouse',
    displayName: 'Greenhouse',
    mode: 'http',
    auth: 'guest',
    listed: true,
    requiresCompany: false,
  },
  {
    id: 'linkedin',
    displayName: 'LinkedIn',
    mode: 'http',
    auth: 'optional',
    listed: true,
    requiresCompany: false,
  },
  {
    id: 'lever',
    displayName: 'Lever',
    mode: 'http',
    auth: 'guest',
    listed: true,
    requiresCompany: true,
  },
  // remoteok: listed=false → should NOT appear in the picker
  {
    id: 'remoteok',
    displayName: 'RemoteOK',
    mode: 'http',
    auth: 'guest',
    listed: false,
    requiresCompany: false,
  },
];

// #621 — a catalog where the selected board carries a curated company seed.
const SEEDED_CATALOG: BoardCatalogEntry[] = [
  {
    id: 'greenhouse',
    displayName: 'Greenhouse',
    mode: 'http',
    auth: 'guest',
    listed: true,
    requiresCompany: true,
    seededCompanies: ['Stripe', 'Airbnb', 'OpenAI', 'Bosch', 'N26', 'Lyft'],
  },
];

const DEFAULT_FORM = {
  boards: ['greenhouse'],
  query: '',
  location: '',
  radiusKm: 0,
  amount: 25,
  dateFilter: '' as const,
  companies: [],
};

const NOOP = () => {};

function renderForm(
  overrides: {
    catalogLoading?: boolean;
    catalog?: BoardCatalogEntry[];
    form?: typeof DEFAULT_FORM;
  } = {}
) {
  stubCatalog = overrides.catalog ?? LISTED_CATALOG;
  stubLoading = overrides.catalogLoading ?? false;

  // Wrap in a minimal fragment — ScrapeForm has no Provider requirements
  function Wrapper({ children }: { children: ReactNode }) {
    return <>{children}</>;
  }

  return render(
    <ScrapeForm
      show={true}
      form={overrides.form ?? DEFAULT_FORM}
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
    // 3 listed boards (greenhouse, linkedin, lever); remoteok is filtered out
    expect(buttons).toHaveLength(3);
  });

  it('does NOT render a button for remoteok (listed=false)', () => {
    renderForm();
    expect(screen.queryByText('jobs.boards.remoteok')).not.toBeInTheDocument();
  });

  it('renders greenhouse (guest board) in the picker', () => {
    renderForm();
    expect(screen.getByText('jobs.boards.greenhouse')).toBeInTheDocument();
  });

  it('renders linkedin (optional board) in the picker', () => {
    renderForm();
    expect(screen.getByText('jobs.boards.linkedin')).toBeInTheDocument();
  });

  it('renders lever (guest board) in the picker', () => {
    renderForm();
    expect(screen.getByText('jobs.boards.lever')).toBeInTheDocument();
  });

  it('renders boards in catalog (registry) order', () => {
    renderForm();
    const buttons = screen
      .getAllByRole('button', { hidden: false })
      .filter((el) => el.getAttribute('aria-pressed') !== null);
    const labels = buttons.map((r) => r.textContent ?? '');
    expect(labels[0]).toBe('jobs.boards.greenhouse');
    expect(labels[1]).toBe('jobs.boards.linkedin');
    expect(labels[2]).toBe('jobs.boards.lever');
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

// ---------------------------------------------------------------------------
// LocationFilterNote integration (PR F) — real component, not stubbed here,
// so a wrong prop name at the ScrapeForm call site would fail this render
// instead of silently compiling and passing every other test in the file.
// ---------------------------------------------------------------------------

describe('ScrapeForm — location filter note (PR F integration)', () => {
  it('shows the note when a location is set and the selected board does not support it', () => {
    // greenhouse (LISTED_CATALOG) has no `supportsLocation` — falsy, non-supporting.
    renderForm({ form: { ...DEFAULT_FORM, boards: ['greenhouse'], location: 'Berlin' } });
    expect(screen.getByRole('note')).toBeInTheDocument();
  });

  it('hides the note when no location is set (default form)', () => {
    renderForm({ form: { ...DEFAULT_FORM, boards: ['greenhouse'], location: '' } });
    expect(screen.queryByRole('note')).not.toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// SeededCompaniesNote integration (#621) — real component, not stubbed here,
// so a wrong prop name at the ScrapeForm call site would fail this render
// instead of silently compiling and passing every other test in the file.
// No location set in either case, so LocationFilterNote stays out of the way
// and `getByRole('note')` is unambiguous.
// ---------------------------------------------------------------------------

describe('ScrapeForm — seeded companies disclosure (#621 integration)', () => {
  it('shows the disclosure for a selected board with seededCompanies, truncated to 5 names + more', () => {
    renderForm({
      catalog: SEEDED_CATALOG,
      form: { ...DEFAULT_FORM, boards: ['greenhouse'], location: '' },
    });
    const note = screen.getByRole('note');
    expect(note.textContent).toContain('Stripe');
    expect(note.textContent).toContain('N26');
    // 6th name truncated away; the pluralized "more" key fired instead (real
    // interpolated count covered by SeededCompaniesNote.i18n.test.ts).
    expect(note.textContent).not.toContain('Lyft');
    expect(note.textContent).toContain('autopilot.wizard.target.seededCompanies.more');
  });

  it('shows no disclosure for a selected board with no seededCompanies', () => {
    // Default LISTED_CATALOG's greenhouse entry carries no seededCompanies.
    renderForm({ form: { ...DEFAULT_FORM, boards: ['greenhouse'], location: '' } });
    expect(screen.queryByRole('note')).not.toBeInTheDocument();
  });
});
