/**
 * SettingsSidebar — structural + search interaction tests.
 *
 * Strategy:
 *  - motion/react is globally shimmed in vitest.setup.ts: motion.span renders
 *    as a plain <span> and forwards all non-motion props, so layoutId becomes
 *    an HTML attribute we can query via getAttribute('layoutId').
 *  - @ajh/ui is NOT mocked; we render the real NavPill / Button / EmptyState /
 *    Input to exercise the actual component tree.
 *  - No IPC / QueryClient needed: SettingsSidebar is pure presentational.
 *
 * Covers (feat/accent-gradients — existing):
 *  - Active row renders a motion.span (shimmed to <span>) carrying text-brand-soft
 *    that wraps the ChevronRight svg icon.
 *  - The chevron span is the sole text-brand-soft element that contains a child svg.
 *  - Inactive rows do NOT render a chevron span.
 *  - onSectionChange fires with the correct id on row click.
 *  - Active row has aria-current="page"; inactive rows have none.
 *
 * Covers (Settings-search feature — new):
 *  - Input role=combobox + aria-autocomplete + aria-expanded + aria-label.
 *  - Empty query: nav groups visible, no listbox.
 *  - Non-empty query: nav groups replaced by role=listbox.
 *  - Listbox aria-label = settings.search.resultsLabel i18n key (returned verbatim by stub).
 *  - Each result row is a role=option li; its Button has tabIndex=-1.
 *  - First result is aria-selected=true (highlighted on mount).
 *  - ArrowDown moves highlight to index 1; result 0 loses aria-selected, result 1 gains it.
 *  - ArrowUp from index 0 wraps to last result.
 *  - Enter on highlighted result calls onResultSelect({section, anchor}) + clears query.
 *  - Enter fallback to onSectionChange when onResultSelect is absent.
 *  - Esc clears query: listbox gone, nav groups back.
 *  - Ctrl+F focuses the input (document.activeElement === input).
 *  - Cmd+F focuses the input on macOS-style keydown.
 *  - No-results: EmptyState rendered (SearchX icon text visible), no listbox.
 *  - aria-live span is sr-only + aria-live=polite + aria-atomic=true.
 *  - aria-live announces resultCount when results present.
 *  - aria-live announces noResultsAria when no results.
 *  - aria-activedescendant on combobox points to the highlighted option id.
 */

import { Cpu, Languages } from 'lucide-react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { NAV_GROUPS, type NavGroup, type SectionId } from '@/features/settings/constants';
import { matchEntries } from '@/features/settings/lib/search';

// ── i18n stub (key-passthrough) ───────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (key: string, params?: Record<string, unknown>) => {
      // Return the key, appending params as a JSON suffix so tests can assert
      // on the key + presence of injected values (e.g. {count}).
      if (params) return `${key}:${JSON.stringify(params)}`;
      return key;
    },
  }),
}));

// ── shared test helpers ───────────────────────────────────────────────────────

/** Key-passthrough t stub — mirrors the mock above. */
const stubT = (key: string) => key;

/**
 * The same sectionLabelKeys the component derives at module level from NAV_GROUPS.
 * Using this ensures matchEntries calls in tests are identical to the component.
 */
const SECTION_LABEL_KEYS = Object.fromEntries(
  NAV_GROUPS.flatMap((g) => g.items.map((item) => [item.id, item.label]))
) as Record<SectionId, string>;

/**
 * Compute expected results using the real matchEntries + stubT, exactly as the
 * component does. Throws if the query produces no results (anti-vacuous guard).
 */
function expectedResults(query: string) {
  const results = matchEntries(query, stubT, SECTION_LABEL_KEYS);
  if (results.length === 0) {
    throw new Error(
      `fixture query '${query}' no longer matches any search entry — update the test query`
    );
  }
  return results;
}

// ── fixtures ──────────────────────────────────────────────────────────────────
//
// NAV_GROUPS_FIXTURE is the navGroups PROP value used only for nav-tree rendering
// tests (chevron, aria-current, click, Esc-restore).  It is intentionally minimal
// (two items) so those tests stay fast and self-contained.
//
// Search tests are NOT driven by this fixture.  The component derives its search
// index at module level from the GLOBAL NAV_GROUPS constant (see SECTION_LABEL_KEYS
// above); the navGroups prop has no effect on search results.  That is why
// expectedResults() / SECTION_LABEL_KEYS both reference the real NAV_GROUPS.

const NAV_GROUPS_FIXTURE: NavGroup[] = [
  {
    label: 'Preferences',
    items: [
      { id: 'general', label: 'General', icon: Languages, description: 'General settings' },
      { id: 'ai', label: 'AI', icon: Cpu, description: 'AI settings' },
    ],
  },
];

// ── component under test (import AFTER mocks) ─────────────────────────────────

import { SettingsSidebar } from './index';

// ── helpers ───────────────────────────────────────────────────────────────────

function renderSidebar(
  activeSection: SectionId = 'general',
  onSectionChange = vi.fn(),
  onResultSelect?: (section: SectionId, anchor: string) => void
) {
  return render(
    <SettingsSidebar
      navGroups={NAV_GROUPS_FIXTURE}
      activeSection={activeSection}
      onSectionChange={onSectionChange}
      onResultSelect={onResultSelect}
    />
  );
}

/** Chevrons: motion.span shimmed to plain span with text-brand-soft class + child svg. */
function findChevrons(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>('.text-brand-soft')).filter(
    (el) => el.querySelector('svg') !== null
  );
}

function getInput(): HTMLInputElement {
  return screen.getByRole('combobox');
}

// ── reset ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.restoreAllMocks();
});

// ═════════════════════════════════════════════════════════════════════════════
// EXISTING TESTS — feat/accent-gradients (preserved)
// ═════════════════════════════════════════════════════════════════════════════

describe('SettingsSidebar — active row chevron (feat/accent-gradients)', () => {
  it('active row renders exactly one chevron span (text-brand-soft + child svg)', () => {
    const { container } = renderSidebar('general');
    expect(findChevrons(container)).toHaveLength(1);
  });

  it('chevron element carries the text-brand-soft class', () => {
    const { container } = renderSidebar('general');
    const [chevron] = findChevrons(container);
    if (!chevron) throw new Error('Expected one chevron element but found none');
    expect(chevron.className).toContain('text-brand-soft');
  });

  it('chevron element contains a ChevronRight svg icon', () => {
    const { container } = renderSidebar('general');
    const [chevron] = findChevrons(container);
    if (!chevron) throw new Error('Expected one chevron element but found none');
    expect(chevron.querySelector('svg')).not.toBeNull();
  });

  it('chevron is absent for all inactive rows', () => {
    const { container } = renderSidebar('general');
    const chevrons = findChevrons(container);
    expect(chevrons).toHaveLength(1);
    const aiButton = screen.getByRole('button', { name: /AI/i });
    const activeChevron = chevrons[0];
    if (!activeChevron) throw new Error('Expected one chevron element but found none');
    expect(aiButton.contains(activeChevron)).toBe(false);
  });

  it('switching active section moves the chevron to the new active row', () => {
    const { container, rerender } = render(
      <SettingsSidebar
        navGroups={NAV_GROUPS_FIXTURE}
        activeSection="general"
        onSectionChange={vi.fn()}
      />
    );
    const generalButton = screen.getByRole('button', { name: /General/i });
    const generalChevron = findChevrons(container)[0];
    if (!generalChevron) throw new Error('Expected chevron in General row but found none');
    expect(generalButton.contains(generalChevron)).toBe(true);

    rerender(
      <SettingsSidebar
        navGroups={NAV_GROUPS_FIXTURE}
        activeSection="ai"
        onSectionChange={vi.fn()}
      />
    );
    expect(findChevrons(container)).toHaveLength(1);
    const aiButton = screen.getByRole('button', { name: /AI/i });
    const aiChevron = findChevrons(container)[0];
    if (!aiChevron) throw new Error('Expected chevron in AI row but found none');
    expect(aiButton.contains(aiChevron)).toBe(true);
  });
});

describe('SettingsSidebar — aria-current', () => {
  it('active row button has aria-current="page"', () => {
    renderSidebar('general');
    expect(screen.getByRole('button', { name: /General/i })).toHaveAttribute(
      'aria-current',
      'page'
    );
  });

  it('inactive row button has no aria-current attribute', () => {
    renderSidebar('general');
    expect(screen.getByRole('button', { name: /AI/i })).not.toHaveAttribute('aria-current');
  });
});

describe('SettingsSidebar — nav interaction', () => {
  it('clicking an inactive row calls onSectionChange with its id', () => {
    const onChange = vi.fn();
    renderSidebar('general', onChange);
    fireEvent.click(screen.getByRole('button', { name: /AI/i }));
    expect(onChange).toHaveBeenCalledOnce();
    expect(onChange).toHaveBeenCalledWith('ai');
  });

  it('clicking the active row still fires onSectionChange', () => {
    const onChange = vi.fn();
    renderSidebar('general', onChange);
    fireEvent.click(screen.getByRole('button', { name: /General/i }));
    expect(onChange).toHaveBeenCalledOnce();
    expect(onChange).toHaveBeenCalledWith('general');
  });
});

// ═════════════════════════════════════════════════════════════════════════════
// NEW TESTS — Settings-search feature
// ═════════════════════════════════════════════════════════════════════════════

describe('SettingsSidebar — search input ARIA attributes', () => {
  it('input has role=combobox', () => {
    renderSidebar();
    expect(getInput()).toHaveAttribute('role', 'combobox');
  });

  it('input has aria-autocomplete="list"', () => {
    renderSidebar();
    expect(getInput()).toHaveAttribute('aria-autocomplete', 'list');
  });

  it('input has aria-label from settings.search.ariaLabel i18n key', () => {
    renderSidebar();
    expect(getInput()).toHaveAttribute('aria-label', 'settings.search.ariaLabel');
  });

  it('aria-expanded is false when query is empty', () => {
    renderSidebar();
    expect(getInput()).toHaveAttribute('aria-expanded', 'false');
  });
});

describe('SettingsSidebar — empty query shows nav groups', () => {
  it('nav groups are visible when query is empty', () => {
    renderSidebar();
    // NAV_GROUPS_FIXTURE (the navGroups prop) has one group labelled "Preferences"
    expect(screen.getByText('Preferences')).toBeInTheDocument();
  });

  it('no listbox is rendered when query is empty', () => {
    renderSidebar();
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
  });
});

describe('SettingsSidebar — query with results', () => {
  it('typing a query removes nav groups and shows a listbox', async () => {
    const user = userEvent.setup();
    renderSidebar();
    await user.type(getInput(), 'theme');
    expect(screen.queryByText('Preferences')).not.toBeInTheDocument();
    expect(screen.getByRole('listbox')).toBeInTheDocument();
  });

  it('listbox has aria-label = settings.search.resultsLabel', async () => {
    const user = userEvent.setup();
    renderSidebar();
    await user.type(getInput(), 'theme');
    expect(screen.getByRole('listbox')).toHaveAttribute(
      'aria-label',
      'settings.search.resultsLabel'
    );
  });

  it('result count and first-row title match matchEntries output for "theme"', async () => {
    const query = 'theme';
    const expected = expectedResults(query); // throws if no match (anti-vacuous guard)
    const user = userEvent.setup();
    renderSidebar();
    await user.type(getInput(), query);
    const options = screen.getAllByRole('option');
    expect(options).toHaveLength(expected.length);
    // First result title must be the resolved titleKey (key passthrough in stub)
    const firstExpected = expected[0];
    if (!firstExpected) throw new Error('expected at least one result');
    expect(options[0]).toHaveTextContent(firstExpected.title);
  });

  it('each result row is a role=option <li>', async () => {
    const user = userEvent.setup();
    renderSidebar();
    await user.type(getInput(), 'theme');
    const options = screen.getAllByRole('option');
    expect(options.length).toBeGreaterThan(0);
  });

  it('each option Button has tabIndex=-1', async () => {
    const user = userEvent.setup();
    const { container } = renderSidebar();
    await user.type(getInput(), 'theme');
    const listbox = screen.getByRole('listbox');
    // All buttons inside result options must have tabIndex -1
    const buttons = Array.from(listbox.querySelectorAll<HTMLElement>('button'));
    expect(buttons.length).toBeGreaterThan(0);
    for (const btn of buttons) {
      expect(btn.tabIndex).toBe(-1);
    }
    void container; // silence lint
  });

  it('first result has aria-selected=true', async () => {
    const user = userEvent.setup();
    renderSidebar();
    await user.type(getInput(), 'theme');
    const options = screen.getAllByRole('option');
    const first = options[0];
    if (!first) throw new Error('expected at least one option');
    expect(first).toHaveAttribute('aria-selected', 'true');
  });

  it('subsequent results have aria-selected=false (language matches ≥2 entries)', async () => {
    const query = 'language';
    const expected = expectedResults(query); // throws if no match (anti-vacuous guard)
    if (expected.length < 2) {
      throw new Error(
        `fixture query '${query}' must match ≥2 entries to test multi-result selection; update the query`
      );
    }
    const user = userEvent.setup();
    renderSidebar();
    await user.type(getInput(), query);
    const options = screen.getAllByRole('option');
    expect(options).toHaveLength(expected.length);
    const second = options[1];
    if (!second) throw new Error('expected at least two options');
    expect(second).toHaveAttribute('aria-selected', 'false');
  });

  it('aria-expanded is true when results are present', async () => {
    const user = userEvent.setup();
    renderSidebar();
    await user.type(getInput(), 'theme');
    expect(getInput()).toHaveAttribute('aria-expanded', 'true');
  });

  it('aria-activedescendant on combobox points to the highlighted option id', async () => {
    const user = userEvent.setup();
    renderSidebar();
    await user.type(getInput(), 'theme');
    const input = getInput();
    const descendant = input.getAttribute('aria-activedescendant');
    expect(descendant).toBeTruthy();
    // The id must exist in the DOM
    if (!descendant) throw new Error('aria-activedescendant is null/empty');
    const el = document.getElementById(descendant);
    expect(el).not.toBeNull();
    expect(el?.getAttribute('role')).toBe('option');
  });
});

describe('SettingsSidebar — keyboard: ArrowDown', () => {
  it('ArrowDown moves highlight from index 0 to index 1', async () => {
    const user = userEvent.setup();
    renderSidebar();
    // Use a broad query to ensure ≥2 results
    await user.type(getInput(), 'a');
    const optionsBefore = screen.getAllByRole('option');
    expect(optionsBefore.length).toBeGreaterThanOrEqual(2); // fixture drift guard: need ≥2 results
    expect(optionsBefore[0]).toHaveAttribute('aria-selected', 'true');
    expect(optionsBefore[1]).toHaveAttribute('aria-selected', 'false');

    await user.keyboard('{ArrowDown}');

    const optionsAfter = screen.getAllByRole('option');
    expect(optionsAfter[0]).toHaveAttribute('aria-selected', 'false');
    expect(optionsAfter[1]).toHaveAttribute('aria-selected', 'true');
  });

  it('ArrowDown on the last result wraps to index 0', async () => {
    const user = userEvent.setup();
    renderSidebar();
    await user.type(getInput(), 'a');
    const options = screen.getAllByRole('option');
    expect(options.length).toBeGreaterThanOrEqual(2); // fixture drift guard: need ≥2 results
    // Move highlight to last
    for (let i = 0; i < options.length - 1; i++) {
      await user.keyboard('{ArrowDown}');
    }
    const last = screen.getAllByRole('option');
    expect(last.at(-1)).toHaveAttribute('aria-selected', 'true');

    // One more ArrowDown wraps to first
    await user.keyboard('{ArrowDown}');
    const wrapped = screen.getAllByRole('option');
    expect(wrapped[0]).toHaveAttribute('aria-selected', 'true');
  });
});

describe('SettingsSidebar — keyboard: ArrowUp wraps', () => {
  it('ArrowUp from index 0 wraps highlight to the last result', async () => {
    const user = userEvent.setup();
    renderSidebar();
    await user.type(getInput(), 'a');
    const optionsBefore = screen.getAllByRole('option');
    expect(optionsBefore.length).toBeGreaterThanOrEqual(2); // fixture drift guard: need ≥2 results
    expect(optionsBefore[0]).toHaveAttribute('aria-selected', 'true');

    await user.keyboard('{ArrowUp}');

    const optionsAfter = screen.getAllByRole('option');
    expect(optionsAfter[0]).toHaveAttribute('aria-selected', 'false');
    expect(optionsAfter.at(-1)).toHaveAttribute('aria-selected', 'true');
  });
});

describe('SettingsSidebar — keyboard: Enter selects', () => {
  it('Enter calls onResultSelect with exact {section, anchor} of the first result and clears the query', async () => {
    const query = 'theme';
    const expected = expectedResults(query); // throws if no match (anti-vacuous guard)
    const firstExpected = expected[0];
    if (!firstExpected) throw new Error('expected at least one result for query "theme"');

    const onResultSelect = vi.fn();
    const user = userEvent.setup();
    renderSidebar('general', vi.fn(), onResultSelect);
    await user.type(getInput(), query);

    await user.keyboard('{Enter}');

    expect(onResultSelect).toHaveBeenCalledOnce();
    expect(onResultSelect).toHaveBeenCalledWith(firstExpected.section, firstExpected.anchor);
    // Query must be cleared — no listbox visible
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
  });

  it('Enter calls onSectionChange (fallback) when onResultSelect is not provided', async () => {
    const query = 'theme';
    const firstExpected = expectedResults(query)[0];
    if (!firstExpected) throw new Error('expected at least one result for query "theme"');

    const onSectionChange = vi.fn();
    const user = userEvent.setup();
    renderSidebar('general', onSectionChange, undefined);
    await user.type(getInput(), query);

    await user.keyboard('{Enter}');

    // Must be called with the exact section of the first (highlighted) result.
    const lastCall = onSectionChange.mock.calls.at(-1);
    if (!lastCall) throw new Error('onSectionChange never called');
    expect(lastCall[0]).toBe(firstExpected.section);
    // Query cleared
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
  });
});

describe('SettingsSidebar — keyboard: Escape', () => {
  it('Esc clears the query: listbox gone and nav groups restored', async () => {
    const user = userEvent.setup();
    renderSidebar();
    await user.type(getInput(), 'theme');
    expect(screen.getByRole('listbox')).toBeInTheDocument();

    await user.keyboard('{Escape}');

    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
    expect(screen.getByText('Preferences')).toBeInTheDocument();
    expect(getInput().value).toBe('');
  });
});

describe('SettingsSidebar — keyboard: Ctrl/Cmd+F focuses input', () => {
  it('Ctrl+F focuses the search input', () => {
    renderSidebar();
    const input = getInput();
    // Input starts unfocused
    expect(document.activeElement).not.toBe(input);

    fireEvent.keyDown(window, { key: 'f', ctrlKey: true });

    expect(document.activeElement).toBe(input);
  });

  it('Cmd+F (metaKey) focuses the search input', () => {
    renderSidebar();
    const input = getInput();

    fireEvent.keyDown(window, { key: 'f', metaKey: true });

    expect(document.activeElement).toBe(input);
  });
});

describe('SettingsSidebar — no-results state', () => {
  it('renders EmptyState (no listbox) when query matches nothing', async () => {
    const user = userEvent.setup();
    const { container } = renderSidebar();
    await user.type(getInput(), 'zzznomatchzzz');
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
    // EmptyState renders a <p> whose text content contains the noResults i18n key.
    // Use container.querySelector to avoid the "Found multiple elements" error from
    // screen.getByText when the i18n stub embeds params in the key string.
    const noResultsEl = Array.from(container.querySelectorAll('p')).find((p) =>
      p.textContent?.includes('settings.search.noResults')
    );
    expect(noResultsEl).toBeDefined();
  });

  it('aria-expanded is false when query matches nothing', async () => {
    const user = userEvent.setup();
    renderSidebar();
    await user.type(getInput(), 'zzznomatchzzz');
    expect(getInput()).toHaveAttribute('aria-expanded', 'false');
  });
});

describe('SettingsSidebar — aria-live region', () => {
  it('sr-only aria-live span is always in the DOM', () => {
    const { container } = renderSidebar();
    const live = container.querySelector('[aria-live="polite"]');
    expect(live).not.toBeNull();
    expect(live?.classList.contains('sr-only')).toBe(true);
    expect(live?.getAttribute('aria-atomic')).toBe('true');
  });

  it('aria-live text is empty when query is empty', () => {
    const { container } = renderSidebar();
    const live = container.querySelector('[aria-live="polite"]');
    expect(live?.textContent).toBe('');
  });

  it('aria-live announces resultCount key when results are present', async () => {
    const user = userEvent.setup();
    const { container } = renderSidebar();
    await user.type(getInput(), 'theme');
    const live = container.querySelector('[aria-live="polite"]');
    expect(live?.textContent).toContain('settings.search.resultCount');
  });

  it('aria-live announces noResultsAria key when no results match', async () => {
    const user = userEvent.setup();
    const { container } = renderSidebar();
    await user.type(getInput(), 'zzznomatchzzz');
    const live = container.querySelector('[aria-live="polite"]');
    expect(live?.textContent).toContain('settings.search.noResultsAria');
  });
});

describe('SettingsSidebar — highlighted result classes', () => {
  it('highlighted result button has bg-brand/[0.12] and ring-brand/50 classes', async () => {
    const user = userEvent.setup();
    const { container } = renderSidebar();
    await user.type(getInput(), 'theme');
    const listbox = screen.getByRole('listbox');
    const highlightedOption = listbox.querySelector('[aria-selected="true"]');
    if (!highlightedOption) throw new Error('no highlighted option found');
    const btn = highlightedOption.querySelector('button');
    if (!btn) throw new Error('no button inside highlighted option');
    expect(btn.className).toContain('bg-brand/[0.12]');
    expect(btn.className).toContain('ring-brand/50');
    void container;
  });
});

describe('SettingsSidebar — clicking a result', () => {
  it('clicking a result calls onResultSelect with exact {section, anchor} of the clicked result and clears query', async () => {
    const query = 'theme';
    const expected = expectedResults(query); // throws if no match (anti-vacuous guard)
    const firstExpected = expected[0];
    if (!firstExpected) throw new Error('expected at least one result for query "theme"');

    const onResultSelect = vi.fn();
    const user = userEvent.setup();
    renderSidebar('general', vi.fn(), onResultSelect);
    await user.type(getInput(), query);

    const listbox = screen.getByRole('listbox');
    const firstBtn = listbox.querySelector('button');
    if (!firstBtn) throw new Error('no result button found');
    await user.click(firstBtn);

    expect(onResultSelect).toHaveBeenCalledOnce();
    expect(onResultSelect).toHaveBeenCalledWith(firstExpected.section, firstExpected.anchor);
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
  });
});
