/**
 * ProviderRow — row activation vs. the nested "test key" action.
 *
 * The row is a `role="button"` that expands on click (it cannot be a real
 * `<button>`: that would nest the test-key `<Button>` inside it, which is
 * invalid DOM and what React was reporting as "<button> cannot contain a nested
 * <button>").
 *
 * Unnesting alone does NOT fix the behaviour, which is the trap here: the inner
 * button's click still BUBBLES to the row, so "test key" kept toggling the row's
 * expand state. These pin the guard, not just the markup.
 */
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

import { ProviderRow } from './index';

type Props = React.ComponentProps<typeof ProviderRow>;

const baseProps = (over: Partial<Props> = {}): Props =>
  ({
    provider: 'openai',
    meta: {
      kind: 'cloud',
      label: 'OpenAI',
      description: 'Cloud provider',
      color: 'text-emerald-400',
      docsUrl: 'https://example.com',
    },
    connected: true,
    isActive: false,
    isExpanded: false,
    onToggleExpand: vi.fn(),
    onSetActive: vi.fn(),
    onSelectModel: vi.fn(),
    onOpenDocs: vi.fn(),
    onRecheck: vi.fn(),
    ...over,
  }) as unknown as Props;

describe('ProviderRow — row vs. test-key activation', () => {
  it('does not nest a real <button> inside another <button>', () => {
    // The original defect. Checks actual `<button>` ELEMENTS, not
    // `getAllByRole('button')` — that also matches the `role="button"` row, and
    // a real button inside a role-button row is the repo's accepted pattern
    // (NotificationBell). React's complaint was specifically about tag nesting,
    // which is what makes the DOM invalid.
    const { container } = render(<ProviderRow {...baseProps({ onTestKey: vi.fn() })} />);
    const buttons = container.querySelectorAll('button');
    expect(buttons.length).toBeGreaterThan(0);
    for (const btn of buttons) {
      expect(btn.querySelector('button')).toBeNull();
    }
  });

  it('clicking "test key" does NOT also expand the row', async () => {
    // The behavioural half. Unnesting removes the invalid DOM but the click
    // still bubbles to the row's handler — this is what actually broke for the
    // user, and it survives a markup-only fix.
    const onTestKey = vi.fn();
    const onToggleExpand = vi.fn();
    const user = userEvent.setup();
    render(<ProviderRow {...baseProps({ onTestKey, onToggleExpand })} />);

    const testKey = screen.getAllByRole('button').find((b) => b.className.includes('px-1.5'));
    expect(testKey).toBeDefined();
    await user.click(testKey as HTMLElement);

    expect(onTestKey).toHaveBeenCalledTimes(1);
    expect(onToggleExpand).not.toHaveBeenCalled();
  });

  it('clicking the row itself still expands it', async () => {
    // The differential — a stopPropagation guard that swallowed everything
    // would pass the test above while breaking the row.
    const onToggleExpand = vi.fn();
    const user = userEvent.setup();
    render(<ProviderRow {...baseProps({ onToggleExpand })} />);

    await user.click(screen.getByText('OpenAI'));
    expect(onToggleExpand).toHaveBeenCalledTimes(1);
  });

  it('keeps the row keyboard-activatable', async () => {
    const onToggleExpand = vi.fn();
    const user = userEvent.setup();
    render(<ProviderRow {...baseProps({ onToggleExpand })} />);

    const row = screen.getByText('OpenAI').closest('[role="button"]');
    (row as HTMLElement).focus();
    await user.keyboard('{Enter}');
    expect(onToggleExpand).toHaveBeenCalledTimes(1);
  });
});
