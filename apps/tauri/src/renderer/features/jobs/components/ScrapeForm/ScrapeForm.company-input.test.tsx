/**
 * ScrapeForm — showCompanyInput derivation + comma-parse behavior.
 *
 * Covers:
 *
 * showCompanyInput:
 *   - Selecting a board with requiresCompany:true → companies input rendered.
 *   - Deselecting that board (only non-requiresCompany boards remain) → input hidden.
 *   - Catalog empty / not yet loaded → input hidden, no crash.
 *   - No hardcoded board ids: a novel board id + requiresCompany:true triggers the field.
 *
 * Comma-parse (onFormChange called with parsed array on blur):
 *   - "stripe, airbnb" → ["stripe", "airbnb"]
 *   - ""              → []
 *   - "  "            → []  (whitespace-only → filtered)
 *   - " , "           → []  (whitespace-only segments dropped)
 *   - "  stripe  "    → ["stripe"]  (leading/trailing spaces trimmed)
 *   - Mid-type "stripe, " → raw buffer keeps the trailing comma+space (not clobbered)
 *   - Deselecting the requiresCompany board while companies is non-empty:
 *     the field disappears and the scrape request omits `companies` (no stale value).
 *
 * Strategy: render ScrapeForm with a mocked useBoardsCatalog — same pattern as
 * the sibling ScrapeForm.test.tsx and ScrapeForm.interaction.test.tsx.
 * onFormChange is a vi.fn() so we inspect what the component calls it with.
 */
import { act, type ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import type { BoardCatalogEntry } from '@ajh/shared';

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

vi.mock('./ScrapeFilters', () => ({
  ScrapeFilters: () => <div data-testid="scrape-filters" />,
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
    locale: 'en',
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

/** Returns the companies text input, or null if it is not in the DOM. */
function getCompaniesInput(): HTMLElement | null {
  return document.getElementById('scrape-companies');
}

// ---------------------------------------------------------------------------
// showCompanyInput — field visibility driven by requiresCompany flag
// ---------------------------------------------------------------------------

describe('ScrapeForm — showCompanyInput derivation', () => {
  it('shows the companies input when the selected board has requiresCompany:true', () => {
    renderForm([BOARD_ATS.id]);
    expect(getCompaniesInput()).not.toBeNull();
  });

  it('hides the companies input when only non-requiresCompany boards are selected', () => {
    renderForm([BOARD_PLAIN.id]);
    expect(getCompaniesInput()).toBeNull();
  });

  it('shows the field when a requiresCompany board is among multiple selected boards', () => {
    renderForm([BOARD_PLAIN.id, BOARD_ATS.id]);
    expect(getCompaniesInput()).not.toBeNull();
  });

  it('hides the field after the requiresCompany board is deselected (form reflects new boards)', () => {
    // Render with only the plain board selected — the ATS board has been deselected.
    renderForm([BOARD_PLAIN.id]);
    expect(getCompaniesInput()).toBeNull();
  });

  it('hides the field when the catalog is empty', () => {
    renderForm([BOARD_ATS.id], [], vi.fn(), []);
    expect(getCompaniesInput()).toBeNull();
  });

  it('hides the field while the catalog is loading (no crash)', () => {
    renderForm([BOARD_ATS.id], [], vi.fn(), undefined, true);
    expect(getCompaniesInput()).toBeNull();
  });

  it('does NOT hardcode board ids — a novel requiresCompany board still shows the field', () => {
    // BOARD_ATS has id 'novel-ats-board', which doesn't exist in the real registry.
    // The component must derive visibility from the flag alone.
    renderForm([BOARD_ATS.id]);
    expect(getCompaniesInput()).not.toBeNull();
  });

  it('a requiresCompany:false board in the catalog never shows the field even when selected', () => {
    // Catalog has one board, requiresCompany=false.
    const customCatalog: BoardCatalogEntry[] = [{ ...BOARD_PLAIN, requiresCompany: false }];
    renderForm([BOARD_PLAIN.id], [], vi.fn(), customCatalog);
    expect(getCompaniesInput()).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Comma-parse — onFormChange called with correctly parsed companies array on blur
// ---------------------------------------------------------------------------

/**
 * Types (change) then blurs the companies input and returns the parsed `companies`
 * array that the component passed to onFormChange.
 *
 * Parsing now happens on blur, not on every change event, so this helper fires
 * both events to match the real user interaction.
 */
function changeAndBlurCompaniesInput(
  input: HTMLElement,
  value: string,
  onFormChange: ReturnType<typeof vi.fn>
): string[] {
  onFormChange.mockClear();
  fireEvent.change(input, { target: { value } });
  fireEvent.blur(input, { target: { value } });
  const call = onFormChange.mock.calls[0]?.[0] as { companies?: string[] } | undefined;
  return call?.companies ?? [];
}

describe('ScrapeForm — companies input comma-parse (parsed on blur)', () => {
  it('"stripe, airbnb" is parsed to ["stripe","airbnb"]', () => {
    const onFormChange = vi.fn();
    renderForm([BOARD_ATS.id], [], onFormChange);

    const input = getCompaniesInput();
    if (!input) throw new Error('companies input not found');

    expect(changeAndBlurCompaniesInput(input, 'stripe, airbnb', onFormChange)).toEqual([
      'stripe',
      'airbnb',
    ]);
  });

  it('empty string is parsed to []', () => {
    const onFormChange = vi.fn();
    renderForm([BOARD_ATS.id], ['stripe'], onFormChange);

    const input = getCompaniesInput();
    if (!input) throw new Error('companies input not found');

    expect(changeAndBlurCompaniesInput(input, '', onFormChange)).toEqual([]);
  });

  it('whitespace-only input "  " is parsed to []', () => {
    const onFormChange = vi.fn();
    renderForm([BOARD_ATS.id], [], onFormChange);

    const input = getCompaniesInput();
    if (!input) throw new Error('companies input not found');

    expect(changeAndBlurCompaniesInput(input, '  ', onFormChange)).toEqual([]);
  });

  it('" , " (comma with only spaces around it) is parsed to []', () => {
    const onFormChange = vi.fn();
    renderForm([BOARD_ATS.id], [], onFormChange);

    const input = getCompaniesInput();
    if (!input) throw new Error('companies input not found');

    expect(changeAndBlurCompaniesInput(input, ' , ', onFormChange)).toEqual([]);
  });

  it('leading/trailing spaces are trimmed: "  stripe  " → ["stripe"]', () => {
    const onFormChange = vi.fn();
    renderForm([BOARD_ATS.id], [], onFormChange);

    const input = getCompaniesInput();
    if (!input) throw new Error('companies input not found');

    expect(changeAndBlurCompaniesInput(input, '  stripe  ', onFormChange)).toEqual(['stripe']);
  });

  it('mid-type trailing comma+space is preserved in the raw buffer (not snapped back)', () => {
    // Regression: previously onChange re-derived value from companies.join(', '),
    // which turned "stripe, " back into "stripe" and broke multi-company typing.
    // Now the raw string is the controlled value; parsing only happens on blur.
    const onFormChange = vi.fn();
    renderForm([BOARD_ATS.id], [], onFormChange);

    const input = getCompaniesInput();
    if (!input) throw new Error('companies input not found');

    // Simulate user typing "stripe, " (trailing comma+space — second company not yet typed).
    fireEvent.change(input, { target: { value: 'stripe, ' } });

    // onFormChange must NOT be called mid-type (parsing deferred to blur).
    expect(onFormChange).not.toHaveBeenCalled();

    // The input's displayed value must retain the trailing ", " exactly.
    expect((input as HTMLInputElement).value).toBe('stripe, ');
  });
});

// ---------------------------------------------------------------------------
// a11y — input is associated with its hint paragraph
// ---------------------------------------------------------------------------

describe('ScrapeForm — companies input a11y', () => {
  it('input has aria-describedby pointing to the hint paragraph', () => {
    renderForm([BOARD_ATS.id]);

    const input = getCompaniesInput();
    if (!input) throw new Error('companies input not found');

    expect(input.getAttribute('aria-describedby')).toBe('scrape-companies-hint');
    expect(document.getElementById('scrape-companies-hint')).not.toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Stale-value guard — when requiresCompany board is deselected, field disappears
// and the form state no longer carries companies to the scrape request.
// ---------------------------------------------------------------------------

describe('ScrapeForm — companies field absent after deselecting requiresCompany board', () => {
  it('companies input is not in the DOM when only non-requiresCompany boards are selected', () => {
    // Simulate the state AFTER a user deselected the ATS board.
    // The parent controls form.boards; when boards=[BOARD_PLAIN.id], showCompanyInput=false.
    renderForm([BOARD_PLAIN.id], ['stripe']);

    // The field must be hidden — the stale company value cannot be submitted.
    expect(getCompaniesInput()).toBeNull();
    // The label text must also be absent.
    expect(screen.queryByText('jobs.companies.label')).toBeNull();
  });

  it('clears companies via onFormChange when showCompanyInput transitions to false', async () => {
    // Render with an ATS board selected and companies pre-filled.
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

    // Deselect the ATS board → showCompanyInput becomes false.
    // Wrap in act() so the useEffect that clears companies flushes synchronously.
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

    // The clearing effect must have called onFormChange({ companies: [] }).
    const clearCall = onFormChange.mock.calls.find(
      (c) =>
        Array.isArray((c[0] as { companies?: unknown })?.companies) &&
        (c[0] as { companies: unknown[] }).companies.length === 0
    );
    expect(clearCall).toBeDefined();
  });

  // companies-omission logic is tested at the hook level in:
  //   features/jobs/hooks/useScraping.test.ts
  // (exercising the real useScraping hook, not a local replica).
});
