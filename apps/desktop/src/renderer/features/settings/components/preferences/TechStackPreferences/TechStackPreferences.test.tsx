/**
 * TechStackPreferences — free-text tech-stack add/remove tests.
 *
 * Covers:
 *  1. Typing a custom tech NOT in COMMON_TECH and pressing Enter → mutate called
 *     with category 'other'.
 *  2. Typing a custom tech NOT in COMMON_TECH and clicking the Add button → same.
 *  3. Typing a known tech with different casing → canonical name + category reused.
 *  4. Duplicate rejection (case-insensitive) → mutate NOT called, Add button disabled.
 *  5. Empty/whitespace input → Enter does not call mutate.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// ── service mock ──────────────────────────────────────────────────────────────
// Must be hoisted before the component import so the factory runs first.

const mockMutate = vi.fn();
// `undefined` models the pre-load window (the query hasn't resolved yet).
let mockJobPrefs: { techStack: { name: string; category: string }[] } | undefined = {
  techStack: [],
};

vi.mock('@/services', () => ({
  useJobPreferences: () => ({ data: mockJobPrefs }),
  useSetJobPreferences: () => ({ mutate: mockMutate }),
}));

// ── import component AFTER mocks ──────────────────────────────────────────────

import { TechStackPreferences } from './index';

// ── helpers ───────────────────────────────────────────────────────────────────

function renderComponent() {
  return render(<TechStackPreferences />);
}

/**
 * Returns the Add button (the Plus-icon button inside the add-row flex container).
 *
 * The add-row is `div.flex.items-center.gap-2` that is the DIRECT child of the
 * inputRef wrapper div (which contains only the add-row and nothing else at that
 * level). Individual tech chips also carry `flex items-center gap-2` classes, so
 * we skip those by querying for `div > div.flex.items-center.gap-2` — the add-row
 * is a block-level div whose first child is `div.relative.flex-1` (the input
 * wrapper), which is never present on chip rows. We disambiguate by finding the
 * flex-row that contains the textbox.
 */
function getAddButton(container: HTMLElement): HTMLElement {
  const input = container.querySelector<HTMLElement>('input[type="text"]');
  if (!input) throw new Error('textbox input not found');
  // Walk up to the add-row div (the flex container that is an ancestor of the input)
  const addRow = input.closest<HTMLElement>('.flex.items-center.gap-2');
  if (!addRow)
    throw new Error('add-row not found — expected .flex.items-center.gap-2 ancestor of input');
  return within(addRow).getByRole('button');
}

// ── reset ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  mockMutate.mockClear();
  mockJobPrefs = { techStack: [] };
});

// ── tests ─────────────────────────────────────────────────────────────────────

describe('TechStackPreferences — adding a custom tech via Enter', () => {
  it('calls mutate with category "other" for a tech not in COMMON_TECH', async () => {
    const user = userEvent.setup();
    const { container } = renderComponent();

    const input = screen.getByRole('textbox');
    await user.type(input, 'MyCustomFramework{Enter}');

    expect(mockMutate).toHaveBeenCalledTimes(1);
    const call = mockMutate.mock.calls.at(0);
    if (!call) throw new Error('expected mockMutate to have been called');
    const payload = call[0] as { techStack: { name: string; category: string }[] };
    expect(payload.techStack).toContainEqual({ name: 'MyCustomFramework', category: 'other' });
    // suppress unused-var lint — container used implicitly via closure in helper
    void container;
  });
});

describe('TechStackPreferences — adding a custom tech via the Add button', () => {
  it('calls mutate with category "other" when the Add button is clicked', async () => {
    const user = userEvent.setup();
    const { container } = renderComponent();

    const input = screen.getByRole('textbox');
    await user.type(input, 'AnotherCustomLib');

    const addBtn = getAddButton(container);
    await user.click(addBtn);

    expect(mockMutate).toHaveBeenCalledTimes(1);
    const call = mockMutate.mock.calls.at(0);
    if (!call) throw new Error('expected mockMutate to have been called');
    const payload = call[0] as { techStack: { name: string; category: string }[] };
    expect(payload.techStack).toContainEqual({ name: 'AnotherCustomLib', category: 'other' });
  });
});

describe('TechStackPreferences — known tech canonical lookup', () => {
  it('uses canonical name and category when input matches COMMON_TECH case-insensitively', async () => {
    const user = userEvent.setup();
    renderComponent();

    const input = screen.getByRole('textbox');
    // 'typescript' (all lower) should resolve to { name: 'TypeScript', category: 'language' }
    await user.type(input, 'typescript{Enter}');

    expect(mockMutate).toHaveBeenCalledTimes(1);
    const call = mockMutate.mock.calls.at(0);
    if (!call) throw new Error('expected mockMutate to have been called');
    const payload = call[0] as { techStack: { name: string; category: string }[] };
    expect(payload.techStack).toContainEqual({ name: 'TypeScript', category: 'language' });
  });
});

describe('TechStackPreferences — duplicate rejection', () => {
  it('does not call mutate and Add button is disabled when input duplicates an existing tech (case-insensitive)', async () => {
    // Seed the tech stack with React already present.
    mockJobPrefs = { techStack: [{ name: 'React', category: 'framework' }] };

    const user = userEvent.setup();
    const { container } = renderComponent();

    const input = screen.getByRole('textbox');
    // Type "react" (different casing) — a case-insensitive duplicate of 'React'.
    await user.type(input, 'react');

    // The Add button should be disabled because of the duplicate.
    const addBtn = getAddButton(container);
    expect(addBtn).toBeDisabled();

    // Pressing Enter should also be a no-op.
    await user.keyboard('{Enter}');

    expect(mockMutate).not.toHaveBeenCalled();
  });
});

describe('TechStackPreferences — empty / whitespace input', () => {
  it('does not call mutate when Enter is pressed on an empty input', async () => {
    const user = userEvent.setup();
    renderComponent();

    const input = screen.getByRole('textbox');
    await user.click(input);
    await user.keyboard('{Enter}');

    expect(mockMutate).not.toHaveBeenCalled();
  });

  it('does not call mutate when input contains only whitespace', async () => {
    const user = userEvent.setup();
    renderComponent();

    const input = screen.getByRole('textbox');
    // userEvent.type types each character; spaces won't trigger enter.
    await user.type(input, '   {Enter}');

    expect(mockMutate).not.toHaveBeenCalled();
  });
});

describe('TechStackPreferences — pre-load guard (CodeRabbit #756)', () => {
  it('does not call the full-row mutate before job preferences have loaded', async () => {
    // A `{...undefined, techStack}` write would NULL every other column
    // (location, countryCode, salaryExpectation, extraAgencyCompanies).
    mockJobPrefs = undefined;
    const user = userEvent.setup();
    renderComponent();

    const input = screen.getByRole('textbox');
    await user.type(input, 'Rust{Enter}');

    expect(mockMutate).not.toHaveBeenCalled();
  });
});
