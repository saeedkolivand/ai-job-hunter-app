import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { RichTextEditor } from './RichTextEditor';

/**
 * Make a real document change through the real editor. Typing into the
 * ProseMirror surface needs layout APIs jsdom does not implement, but a toolbar
 * command dispatches a genuine transaction — which is all `onUpdate` needs.
 */
async function toggleBulletList(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('button', { name: 'Bullet list' }));
}

/**
 * Loading the initial content is itself a transaction, so a freshly mounted
 * editor schedules one debounced emit of the *unchanged* value (this happens on
 * `main` too). Wait for that mount emit and discard it, so a later
 * "nothing emitted yet" assertion is not racing it on a slow runner — the flake
 * that made the previous version of this suite unreliable in CI.
 */
async function drainMountEmit(onChange: ReturnType<typeof vi.fn>) {
  await waitFor(() => expect(onChange).toHaveBeenCalled());
  onChange.mockClear();
}

describe('RichTextEditor — pending edit on unmount', () => {
  it('flushes a debounced edit when the editor unmounts', async () => {
    // Preview / Edit / Source is a conditional render, so switching view
    // unmounts this component. Everything changed inside the last
    // ONCHANGE_DEBOUNCE_MS used to be dropped silently: the cleanup cleared the
    // timer and never emitted.
    const user = userEvent.setup();
    const onChange = vi.fn();
    const { unmount } = render(<RichTextEditor value="Hello" onChange={onChange} />);
    await drainMountEmit(onChange);

    await toggleBulletList(user);

    // Unmount inside the debounce window, before the edit's own timer fires.
    expect(onChange).not.toHaveBeenCalled();
    unmount();

    // The unmount flush is synchronous (it serializes the captured doc directly,
    // not via the timer), so onChange has already fired exactly once.
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange.mock.calls[0]?.[0]).toContain('Hello');
  });

  it('never emits changed content for an untouched document', async () => {
    // The flush must be value-PRESERVING for an untouched document — otherwise a
    // plain view switch would mark it dirty with different content. Both the
    // mount emit and any unmount flush of the untouched doc must round-trip to
    // exactly the input value.
    const onChange = vi.fn();
    const { unmount } = render(<RichTextEditor value="Hello" onChange={onChange} />);

    await screen.findByRole('button', { name: 'Bullet list' });
    await waitFor(() => expect(onChange).toHaveBeenCalled());
    unmount();

    for (const [md] of onChange.mock.calls) {
      expect(md).toBe('Hello');
    }
  });

  it('still emits exactly once when the debounce fires normally', async () => {
    // The pending-doc handoff must not double-emit: the timer path clears the
    // ref, so the unmount flush that follows finds nothing left to send.
    const user = userEvent.setup();
    const onChange = vi.fn();
    const { unmount } = render(<RichTextEditor value="Hello" onChange={onChange} />);
    await drainMountEmit(onChange);

    await toggleBulletList(user);
    await waitFor(() => expect(onChange).toHaveBeenCalledTimes(1)); // debounce fired

    unmount(); // the ref was cleared by the timer, so cleanup emits nothing

    expect(onChange).toHaveBeenCalledTimes(1);
  });
});
