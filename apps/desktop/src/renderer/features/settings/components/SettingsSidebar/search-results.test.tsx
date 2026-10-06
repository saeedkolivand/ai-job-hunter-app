/**
 * SettingsSidebar — search interaction tests (results listbox, keyboard navigation,
 * selection, click). Structural tests live in `index.test.tsx`.
 *
 * Covers: result listbox + option roles, aria-selected highlighting, ArrowDown/Up
 * wrap, Enter → onResultSelect / onSectionChange fallback, Esc clears the query,
 * Ctrl/Cmd+F focuses the input, aria-activedescendant, highlighted classes, click.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { expectedResults, getInput, renderSidebar, searchFor } from './test-support';

vi.mock('@ajh/translations', async () => (await import('./translations-stub')).translationsMock);

beforeEach(() => {
  vi.restoreAllMocks();
});

describe('SettingsSidebar — query with results', () => {
  it('typing a query removes nav groups and shows a listbox', async () => {
    await searchFor('theme');
    expect(screen.queryByText('Preferences')).not.toBeInTheDocument();
    expect(screen.getByRole('listbox')).toBeInTheDocument();
  });

  it('listbox has aria-label = settings.search.resultsLabel', async () => {
    await searchFor('theme');
    expect(screen.getByRole('listbox')).toHaveAttribute(
      'aria-label',
      'settings.search.resultsLabel'
    );
  });

  it('result count and first-row title match matchEntries output for "theme"', async () => {
    const query = 'theme';
    const expected = expectedResults(query); // throws if no match (anti-vacuous guard)
    await searchFor(query);
    const options = screen.getAllByRole('option');
    expect(options).toHaveLength(expected.length);
    // First result title must be the resolved titleKey (key passthrough in stub)
    const firstExpected = expected[0];
    if (!firstExpected) throw new Error('expected at least one result');
    expect(options[0]).toHaveTextContent(firstExpected.title);
  });

  it('each result row is a role=option <li>', async () => {
    await searchFor('theme');
    const options = screen.getAllByRole('option');
    expect(options.length).toBeGreaterThan(0);
  });

  it('each option Button has tabIndex=-1', async () => {
    const { container } = await searchFor('theme');
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
    await searchFor('theme');
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
    await searchFor(query);
    const options = screen.getAllByRole('option');
    expect(options).toHaveLength(expected.length);
    const second = options[1];
    if (!second) throw new Error('expected at least two options');
    expect(second).toHaveAttribute('aria-selected', 'false');
  });

  it('aria-expanded is true when results are present', async () => {
    await searchFor('theme');
    expect(getInput()).toHaveAttribute('aria-expanded', 'true');
  });

  it('aria-activedescendant on combobox points to the highlighted option id', async () => {
    await searchFor('theme');
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
    const { user } = await searchFor('a');
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
    const { user } = await searchFor('a');
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
    const { user } = await searchFor('theme');
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

describe('SettingsSidebar — highlighted result classes', () => {
  it('highlighted result button has bg-brand/[0.12] and ring-brand/50 classes', async () => {
    const { container } = await searchFor('theme');
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
