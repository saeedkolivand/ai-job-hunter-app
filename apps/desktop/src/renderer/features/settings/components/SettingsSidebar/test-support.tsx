/**
 * Shared fixtures for the SettingsSidebar suites (no `.test.` in the name, so vitest
 * does not collect it).
 */
import { Cpu, Languages } from 'lucide-react';
import { vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { NAV_GROUPS, type NavGroup, type SectionId } from '@/features/settings/constants';
import { matchEntries } from '@/features/settings/lib/search';

import { SettingsSidebar } from './index';

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
export function expectedResults(query: string) {
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

export const NAV_GROUPS_FIXTURE: NavGroup[] = [
  {
    label: 'Preferences',
    items: [
      { id: 'general', label: 'General', icon: Languages, description: 'General settings' },
      { id: 'ai', label: 'AI', icon: Cpu, description: 'AI settings' },
    ],
  },
];

export function renderSidebar(
  activeSection: SectionId = 'general',
  onSectionChange: (section: SectionId) => void = vi.fn(),
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
export function findChevrons(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>('.text-brand-soft')).filter(
    (el) => el.querySelector('svg') !== null
  );
}

export function getInput(): HTMLInputElement {
  return screen.getByRole('combobox');
}

/** Render the sidebar and type `query` into the search box. */
export async function searchFor(
  query: string
): Promise<{ user: ReturnType<typeof userEvent.setup> } & ReturnType<typeof renderSidebar>> {
  const user = userEvent.setup();
  const result = renderSidebar();
  await user.type(getInput(), query);
  return { user, ...result };
}
