/**
 * SettingsSidebar — structural tests (feat/accent-gradients).
 *
 * Strategy:
 *  - motion/react is globally shimmed in vitest.setup.ts: motion.span renders
 *    as a plain <span> and forwards all non-motion props, so layoutId becomes
 *    an HTML attribute we can query via getAttribute('layoutId').
 *  - @ajh/ui is NOT mocked; we render the real NavPill / Button to exercise
 *    the actual component tree.
 *  - No IPC / QueryClient needed: SettingsSidebar is pure presentational.
 *
 * Covers (feat/accent-gradients):
 *  - Active row renders a motion.span (shimmed to <span>) carrying text-brand-soft
 *    that wraps the ChevronRight svg icon.
 *  - The chevron span is the sole text-brand-soft element that contains a child svg.
 *  - Inactive rows do NOT render a chevron span.
 *  - onSectionChange fires with the correct id on row click.
 *  - Active row has aria-current="page"; inactive rows have none.
 */
// ── fixtures ──────────────────────────────────────────────────────────────────
// Minimal nav groups: one group with two items so we can test active vs inactive.
import { Cpu, Languages } from 'lucide-react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import type { NavGroup, SectionId } from '@/features/settings/constants';

import { SettingsSidebar } from './index';

const NAV_GROUPS_FIXTURE: NavGroup[] = [
  {
    label: 'Preferences',
    items: [
      {
        id: 'general',
        label: 'General',
        icon: Languages,
        description: 'General settings',
      },
      {
        id: 'ai',
        label: 'AI',
        icon: Cpu,
        description: 'AI settings',
      },
    ],
  },
];

// ── helpers ───────────────────────────────────────────────────────────────────

function renderSidebar(activeSection: SectionId, onSectionChange = vi.fn()) {
  return render(
    <SettingsSidebar
      navGroups={NAV_GROUPS_FIXTURE}
      activeSection={activeSection}
      onSectionChange={onSectionChange}
    />
  );
}

/**
 * Find all chevron spans in the sidebar.
 *
 * The motion shim strips layoutId (it is in MOTION_PROPS) before passing props
 * to the underlying DOM element, so we cannot query by [layoutid=…].
 * Instead we rely on the stable className "text-brand-soft" that SettingsSidebar
 * unconditionally places on the motion.span that wraps ChevronRight. That class
 * is unique to the chevron element inside the sidebar — no other element in the
 * tree carries it — so it is a reliable structural hook.
 */
function findChevrons(container: HTMLElement): HTMLElement[] {
  // The motion.span renders as a plain <span>; its className contains text-brand-soft.
  // We match any element (span or otherwise) carrying that class so the test is
  // resilient to the tag name the shim chooses.
  return Array.from(container.querySelectorAll<HTMLElement>('.text-brand-soft')).filter(
    (el) => el.querySelector('svg') !== null
  );
}

// ── tests ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.restoreAllMocks();
});

describe('SettingsSidebar — active row chevron (feat/accent-gradients)', () => {
  it('active row renders exactly one chevron span (text-brand-soft + child svg)', () => {
    const { container } = renderSidebar('general');
    const chevrons = findChevrons(container);
    expect(chevrons).toHaveLength(1);
  });

  it('chevron element carries the text-brand-soft class', () => {
    const { container } = renderSidebar('general');
    const [chevron] = findChevrons(container);
    expect(chevron.className).toContain('text-brand-soft');
  });

  it('chevron element contains a ChevronRight svg icon', () => {
    const { container } = renderSidebar('general');
    const [chevron] = findChevrons(container);
    // lucide icons render as <svg> elements
    const svg = chevron.querySelector('svg');
    expect(svg).not.toBeNull();
  });

  it('chevron is absent for all inactive rows', () => {
    // 'general' is active → 'ai' row is inactive and must have no chevron
    const { container } = renderSidebar('general');
    const chevrons = findChevrons(container);
    // Only the active row gets a chevron
    expect(chevrons).toHaveLength(1);
    // Verify the single chevron is inside the General row, not the AI row
    const aiButton = screen.getByRole('button', { name: /AI/i });
    expect(aiButton.contains(chevrons[0])).toBe(false);
  });

  it('switching active section moves the chevron to the new active row', () => {
    const { container, rerender } = render(
      <SettingsSidebar
        navGroups={NAV_GROUPS_FIXTURE}
        activeSection="general"
        onSectionChange={vi.fn()}
      />
    );
    expect(findChevrons(container)).toHaveLength(1);
    const generalButton = screen.getByRole('button', { name: /General/i });
    expect(generalButton.contains(findChevrons(container)[0])).toBe(true);

    rerender(
      <SettingsSidebar
        navGroups={NAV_GROUPS_FIXTURE}
        activeSection="ai"
        onSectionChange={vi.fn()}
      />
    );
    expect(findChevrons(container)).toHaveLength(1);
    const aiButton = screen.getByRole('button', { name: /AI/i });
    expect(aiButton.contains(findChevrons(container)[0])).toBe(true);
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

describe('SettingsSidebar — interaction', () => {
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
