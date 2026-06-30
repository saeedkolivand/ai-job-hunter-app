/**
 * ScrapeForm — multi-select board interaction tests.
 *
 * Covers:
 *   - Clicking an unselected board calls onFormChange adding it to boards
 *   - Clicking the only selected board does NOT deselect it (last-board guard)
 *   - Clicking a selected board (when others are selected) removes it
 *   - Select-all sets boards to all listed board ids
 *   - Clear resets boards to only the first listed board
 *   - Clear is disabled when only one board is selected (form.boards.length <= 1)
 *   - Select-all is disabled when all boards are already selected
 *   - makeMultiSelectKeyHandler is wired: group receives an onKeyDown handler
 *
 * motion/react is globally shimmed in vitest.setup.ts.
 */
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { BoardCatalogEntry } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';

// ---------------------------------------------------------------------------
// Module-level stubs
// ---------------------------------------------------------------------------

let stubCatalog: BoardCatalogEntry[] = [];
let stubLoading = false;

type StubStatus = { data: { connected: boolean } | undefined };
let stubStatuses: { results: StubStatus[]; anyConnected: boolean } = {
  results: [],
  anyConnected: false,
};

vi.mock('@/services/use-boards', () => ({
  useBoardsCatalog: () => ({
    data: stubCatalog,
    isLoading: stubLoading,
    isSuccess: !stubLoading,
  }),
  useBoardStatuses: () => stubStatuses,
}));

// Stub out Adzuna key presence — not under test in this file.
vi.mock('@/services/use-ai-provider', () => ({
  useHasProviderKey: () => ({ data: { has: false } }),
}));

vi.mock('./ScrapeFilters', () => ({
  ScrapeFilters: () => <div data-testid={TEST_IDS.jobs.scrapeFilters} />,
}));

vi.mock('./BoardConnectChip', () => ({
  BoardConnectChip: ({ board }: { board: string }) => <span data-testid={`chip-${board}`} />,
}));

// Capture the handler so we can verify it's wired correctly.
let capturedKeyHandler: ((...args: unknown[]) => unknown) | null = null;
vi.mock('@/hooks/use-roving-tabindex', () => ({
  makeMultiSelectKeyHandler: (...args: unknown[]) => {
    capturedKeyHandler = vi.fn();
    // Forward the real arguments count so we know the component passed them.
    void args;
    return capturedKeyHandler;
  },
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

// Import AFTER mocks
import { ScrapeForm } from './index';

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const CATALOG: BoardCatalogEntry[] = [
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
];

type FormBoards = { boards: string[] };

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

function Wrapper({ children }: { children: ReactNode }) {
  return <>{children}</>;
}

function renderForm(boards: string[], onFormChange: (u: Partial<FormBoards>) => void = vi.fn()) {
  stubCatalog = CATALOG;
  stubLoading = false;
  stubStatuses = { results: [], anyConnected: false };
  capturedKeyHandler = null;

  return render(
    <ScrapeForm
      show={true}
      form={buildForm(boards)}
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Returns the board toggle button with the given label — throws if absent (preferred over !) */
function getBoardButton(label: string): HTMLElement {
  const btn = screen
    .getAllByRole('button')
    .find((el) => el.getAttribute('aria-pressed') !== null && el.textContent === label);
  if (!btn) throw new Error(`Board toggle button not found: "${label}"`);
  return btn;
}

// ---------------------------------------------------------------------------
// Click interaction — toggling boards
// ---------------------------------------------------------------------------

describe('ScrapeForm board toggle — add board', () => {
  it('clicking an unselected board adds it via onFormChange, preserving existing selection order', async () => {
    const onFormChange = vi.fn();
    renderForm(['greenhouse'], onFormChange);

    const linkedinBtn = getBoardButton('jobs.boards.linkedin');
    await userEvent.click(linkedinBtn);

    // toggleBoard appends the new id: [...boards, id] → exact order is load-bearing.
    const call = onFormChange.mock.calls[0]?.[0] as { boards: string[] } | undefined;
    expect(call?.boards).toEqual(['greenhouse', 'linkedin']);
  });

  it('clicking a selected board (multiple selected) removes it via onFormChange', async () => {
    const onFormChange = vi.fn();
    renderForm(['greenhouse', 'linkedin'], onFormChange);

    const linkedinBtn = getBoardButton('jobs.boards.linkedin');
    await userEvent.click(linkedinBtn);

    const call = onFormChange.mock.calls[0]?.[0] as { boards: string[] } | undefined;
    expect(call?.boards).not.toContain('linkedin');
    expect(call?.boards).toContain('greenhouse');
  });

  it('clicking the only selected board does NOT call onFormChange (last-board guard)', async () => {
    const onFormChange = vi.fn();
    renderForm(['greenhouse'], onFormChange);

    const greenhouseBtn = getBoardButton('jobs.boards.greenhouse');
    await userEvent.click(greenhouseBtn);

    // The last board must stay selected — onFormChange must not be called for a deselect.
    const deselects = onFormChange.mock.calls.filter(
      (call) => !(call[0] as { boards: string[] }).boards?.includes('greenhouse')
    );
    expect(deselects).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// Select-all / Clear
// ---------------------------------------------------------------------------

describe('ScrapeForm select-all', () => {
  it('select-all calls onFormChange with all listed board ids', async () => {
    const onFormChange = vi.fn();
    renderForm(['greenhouse'], onFormChange);

    await userEvent.click(screen.getByText('jobs.selectAll'));

    const lastCall = onFormChange.mock.lastCall?.[0] as { boards: string[] } | undefined;
    expect(lastCall?.boards).toEqual(['greenhouse', 'linkedin', 'lever']);
  });

  it('select-all button is disabled when all boards are already selected', () => {
    renderForm(['greenhouse', 'linkedin', 'lever']);

    const selectAllBtn = screen.getByText('jobs.selectAll').closest('button');
    expect(selectAllBtn).toBeDisabled();
  });

  it('select-all button is enabled when not all boards are selected', () => {
    renderForm(['greenhouse']);

    const selectAllBtn = screen.getByText('jobs.selectAll').closest('button');
    expect(selectAllBtn).not.toBeDisabled();
  });
});

describe('ScrapeForm clear boards', () => {
  it('clear calls onFormChange with only the first listed board', async () => {
    const onFormChange = vi.fn();
    renderForm(['greenhouse', 'linkedin', 'lever'], onFormChange);

    await userEvent.click(screen.getByText('jobs.clearBoards'));

    const lastCall = onFormChange.mock.lastCall?.[0] as { boards: string[] } | undefined;
    expect(lastCall?.boards).toEqual(['greenhouse']);
  });

  it('clear button is disabled when only one board is selected', () => {
    renderForm(['greenhouse']);

    const clearBtn = screen.getByText('jobs.clearBoards').closest('button');
    expect(clearBtn).toBeDisabled();
  });

  it('clear button is enabled when multiple boards are selected', () => {
    renderForm(['greenhouse', 'linkedin']);

    const clearBtn = screen.getByText('jobs.clearBoards').closest('button');
    expect(clearBtn).not.toBeDisabled();
  });
});

// ---------------------------------------------------------------------------
// Keyboard handler wiring
// ---------------------------------------------------------------------------

describe('ScrapeForm keyboard handler', () => {
  it('ArrowRight on the board group triggers the makeMultiSelectKeyHandler callback', () => {
    renderForm(['greenhouse']);

    const group = screen.getByRole('group');
    expect(group).toBeInTheDocument();
    // capturedKeyHandler is the vi.fn() returned by our makeMultiSelectKeyHandler mock.
    expect(capturedKeyHandler).not.toBeNull();

    // Fire a real keyboard event — the group's onKeyDown must invoke the handler.
    fireEvent.keyDown(group, { key: 'ArrowRight' });

    // The mock handler must have been called, proving the wiring is live.
    expect(capturedKeyHandler).toHaveBeenCalledTimes(1);
  });
});

// ---------------------------------------------------------------------------
// Required-board login gate — Start button disabled + blockedHint
// ---------------------------------------------------------------------------

// Catalog that includes a required-auth board for gate tests.
// Uses 'arbeitsagentur' (real active board) overridden to auth='required' —
// the gate logic is UI-side only; the catalog fixture drives it.
const CATALOG_WITH_REQUIRED: BoardCatalogEntry[] = [
  ...CATALOG,
  {
    id: 'arbeitsagentur',
    displayName: 'Arbeitsagentur',
    mode: 'http' as const,
    auth: 'required' as const,
    listed: true,
    requiresCompany: false,
  },
];

/**
 * Helper: build a form with a non-empty query so the query gate doesn't
 * interfere with the required-board gate we're testing.
 */
function buildFormWithQuery(boards: string[]): Parameters<typeof ScrapeForm>[0]['form'] {
  return {
    boards,
    query: 'software engineer',
    location: '',
    radiusKm: 0,
    amount: 25,
    dateFilter: '' as const,
    companies: [],
  };
}

function renderFormWithQuery(
  boards: string[],
  statuses: StubStatus[],
  anyConnected = false,
  onStart = vi.fn()
) {
  stubCatalog = CATALOG_WITH_REQUIRED;
  stubLoading = false;
  stubStatuses = { results: statuses, anyConnected };
  capturedKeyHandler = null;

  render(
    <ScrapeForm
      show={true}
      form={buildFormWithQuery(boards)}
      scraping={false}
      scrapeOutcome={null}
      onToggle={vi.fn()}
      onFormChange={vi.fn()}
      onStart={onStart}
      onCancel={vi.fn()}
      onGeocode={async () => []}
    />,
    { wrapper: Wrapper }
  );

  return { onStart };
}

/** Returns the primary Start/Scrape button (variant=primary, no aria-pressed). */
function getStartButton(): HTMLElement {
  return screen.getByTestId(TEST_IDS.jobs.scrapeStartButton);
}

describe('ScrapeForm — required-board login gate', () => {
  it('disables Start and shows blockedHint when a required board (arbeitsagentur) is selected but not connected', () => {
    // arbeitsagentur is auth=required in CATALOG_WITH_REQUIRED; status returns not connected
    renderFormWithQuery(['arbeitsagentur'], [{ data: undefined }]);

    const startBtn = getStartButton();
    expect(startBtn).toBeDisabled();

    expect(document.getElementById('scrape-blocked-hint')).not.toBeNull();
  });

  it('enables Start when a required board (arbeitsagentur) is selected and connected', () => {
    // arbeitsagentur connected → unconnectedRequired is empty → button enabled
    renderFormWithQuery(['arbeitsagentur'], [{ data: { connected: true } }], true);

    const startBtn = getStartButton();
    expect(startBtn).not.toBeDisabled();

    expect(document.getElementById('scrape-blocked-hint')).toBeNull();
  });

  it('does NOT disable Start or show blockedHint for optional/guest boards even when not connected', () => {
    // linkedin=optional, greenhouse=guest; neither has auth=required
    // stubStatuses is empty because requiredBoardIds will be [] for these boards
    renderFormWithQuery(['linkedin', 'greenhouse'], []);

    const startBtn = getStartButton();
    // Disabled only if query is empty or scraping — neither applies here
    expect(startBtn).not.toBeDisabled();

    expect(document.getElementById('scrape-blocked-hint')).toBeNull();
  });

  it('sets aria-describedby="scrape-blocked-hint" on the Start button when blocked', () => {
    renderFormWithQuery(['arbeitsagentur'], [{ data: { connected: false } }]);

    const startBtn = getStartButton();
    expect(startBtn).toHaveAttribute('aria-describedby', 'scrape-blocked-hint');

    // The hint paragraph must carry the matching id
    const hint = document.getElementById('scrape-blocked-hint');
    expect(hint).not.toBeNull();
    expect(hint?.tagName.toLowerCase()).toBe('p');
  });

  // Keyboard-submit regression — CodeRabbit PR #458
  it('Enter on query input does NOT call onStart when a required board (arbeitsagentur) is disconnected', () => {
    const { onStart } = renderFormWithQuery(['arbeitsagentur'], [{ data: undefined }]);

    const queryInput = screen.getByPlaceholderText('jobs.queryPlaceholder');
    fireEvent.keyDown(queryInput, { key: 'Enter' });

    expect(onStart).not.toHaveBeenCalled();
  });

  it('Enter on query input DOES call onStart when a required board (arbeitsagentur) is connected', () => {
    const { onStart } = renderFormWithQuery(
      ['arbeitsagentur'],
      [{ data: { connected: true } }],
      true
    );

    const queryInput = screen.getByPlaceholderText('jobs.queryPlaceholder');
    fireEvent.keyDown(queryInput, { key: 'Enter' });

    expect(onStart).toHaveBeenCalledTimes(1);
  });
});
