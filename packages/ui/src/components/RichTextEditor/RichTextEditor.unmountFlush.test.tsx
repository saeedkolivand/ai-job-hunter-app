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

describe('RichTextEditor — pending edit on unmount', () => {
  it('flushes a debounced edit when the editor unmounts', async () => {
    // Preview / Edit / Source is a conditional render, so switching view
    // unmounts this component. Everything changed inside the last
    // ONCHANGE_DEBOUNCE_MS used to be dropped silently: the cleanup cleared the
    // timer and never emitted.
    const user = userEvent.setup();
    const onChange = vi.fn();
    const { unmount } = render(<RichTextEditor value="Hello" onChange={onChange} />);

    await toggleBulletList(user);

    // Unmount inside the debounce window, before any emit has fired.
    expect(onChange).not.toHaveBeenCalled();
    unmount();

    await waitFor(() => expect(onChange).toHaveBeenCalledTimes(1));
    expect(onChange.mock.calls[0]?.[0]).toContain('Hello');
  });

  it('never emits changed content for an untouched document', async () => {
    // The editor already schedules one emit on mount (pre-existing: loading the
    // initial content is itself a transaction, so a mounted editor emits the
    // unchanged value after the debounce on `main` too). The flush must
    // therefore be value-PRESERVING for an untouched document — otherwise a
    // plain view switch would mark it dirty with different content.
    const onChange = vi.fn();
    const { unmount } = render(<RichTextEditor value="Hello" onChange={onChange} />);

    await screen.findByRole('button', { name: 'Bullet list' });
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

    await toggleBulletList(user);
    await waitFor(() => expect(onChange).toHaveBeenCalledTimes(1));

    unmount();

    expect(onChange).toHaveBeenCalledTimes(1);
  });
});
