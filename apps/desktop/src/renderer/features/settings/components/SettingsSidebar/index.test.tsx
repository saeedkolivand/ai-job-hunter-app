/**
 * SettingsSidebar — structural tests (nav rows, search input ARIA, empty/no-result
 * states, aria-live region). Search interaction (results, keyboard, click) lives in
 * `search-results.test.tsx`.
 *
 * Strategy:
 *  - motion/react is globally shimmed in vitest.setup.ts: motion.span renders
 *    as a plain <span> and forwards all non-motion props.
 *  - @ajh/ui is NOT mocked; we render the real NavPill / Button / EmptyState /
 *    Input to exercise the actual component tree.
 *  - No IPC / QueryClient needed: SettingsSidebar is pure presentational.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { SettingsSidebar } from './index';
import {
  findChevrons,
  getInput,
  NAV_GROUPS_FIXTURE,
  renderSidebar,
  searchFor,
} from './test-support';

vi.mock('@ajh/translations', async () => (await import('./translations-stub')).translationsMock);

beforeEach(() => {
  vi.restoreAllMocks();
});

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

describe('SettingsSidebar — no-results state', () => {
  it('renders EmptyState (no listbox) when query matches nothing', async () => {
    const { container } = await searchFor('zzznomatchzzz');
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
    await searchFor('zzznomatchzzz');
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
    const { container } = await searchFor('theme');
    const live = container.querySelector('[aria-live="polite"]');
    expect(live?.textContent).toContain('settings.search.resultCount');
  });

  it('aria-live announces noResultsAria key when no results match', async () => {
    const { container } = await searchFor('zzznomatchzzz');
    const live = container.querySelector('[aria-live="polite"]');
    expect(live?.textContent).toContain('settings.search.noResultsAria');
  });
});
