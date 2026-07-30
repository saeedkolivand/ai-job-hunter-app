// @vitest-environment jsdom
import type { KeyboardEvent as ReactKeyboardEvent, MouseEvent as ReactMouseEvent } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { copyChip, copyChipOnKeyDown } from './clipboard';
import { COPIED_TIMEOUT_MS } from './constants';

function stubClipboard(writeText = vi.fn().mockResolvedValue(undefined)) {
  Object.defineProperty(navigator, 'clipboard', {
    value: { writeText },
    configurable: true,
  });
  return writeText;
}

function mouseEvent(el: HTMLElement): ReactMouseEvent<HTMLElement> {
  return { currentTarget: el } as unknown as ReactMouseEvent<HTMLElement>;
}

function keyEvent(key: string, el: HTMLElement): ReactKeyboardEvent<HTMLElement> {
  return {
    key,
    currentTarget: el,
    preventDefault: vi.fn(),
  } as unknown as ReactKeyboardEvent<HTMLElement>;
}

afterEach(() => {
  vi.useRealTimers();
  Reflect.deleteProperty(navigator, 'clipboard');
});

describe('copyChip', () => {
  it('copies data-copy in preference to textContent', () => {
    const writeText = stubClipboard();
    const el = document.createElement('span');
    el.setAttribute('data-copy', 'pnpm install');
    el.textContent = 'install';
    copyChip(mouseEvent(el));
    expect(writeText).toHaveBeenCalledWith('pnpm install');
  });

  it('falls back to textContent when data-copy is absent', () => {
    const writeText = stubClipboard();
    const el = document.createElement('span');
    el.textContent = 'fallback text';
    copyChip(mouseEvent(el));
    expect(writeText).toHaveBeenCalledWith('fallback text');
  });

  it('copies an empty string when neither data-copy nor textContent exist', () => {
    const writeText = stubClipboard();
    const el = document.createElement('span');
    copyChip(mouseEvent(el));
    expect(writeText).toHaveBeenCalledWith('');
  });

  it('is a no-op when navigator.clipboard is unavailable', () => {
    const el = document.createElement('span');
    el.textContent = 'x';
    expect(() => copyChip(mouseEvent(el))).not.toThrow();
    expect(el.classList.contains('copied')).toBe(false);
  });

  it('adds "copied" once the write resolves, then removes it after COPIED_TIMEOUT_MS', async () => {
    vi.useFakeTimers();
    stubClipboard();
    const el = document.createElement('span');
    el.textContent = 'cmd';

    copyChip(mouseEvent(el));
    expect(el.classList.contains('copied')).toBe(false); // writeText promise hasn't resolved yet

    await vi.advanceTimersByTimeAsync(0); // flush the writeText microtask
    expect(el.classList.contains('copied')).toBe(true);

    await vi.advanceTimersByTimeAsync(COPIED_TIMEOUT_MS - 1);
    expect(el.classList.contains('copied')).toBe(true); // not yet — one ms short

    await vi.advanceTimersByTimeAsync(1);
    expect(el.classList.contains('copied')).toBe(false);
  });
});

describe('copyChipOnKeyDown', () => {
  it.each(['Enter', ' '])('activates copyChip and prevents default on "%s"', async (key) => {
    vi.useFakeTimers();
    stubClipboard();
    const el = document.createElement('span');
    el.textContent = 'cmd';
    const event = keyEvent(key, el);

    copyChipOnKeyDown(event);

    expect(event.preventDefault).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(0);
    expect(el.classList.contains('copied')).toBe(true);
  });

  it.each(['Escape', 'Tab', 'a', 'ArrowDown'])('ignores "%s"', (key) => {
    const writeText = stubClipboard();
    const el = document.createElement('span');
    const event = keyEvent(key, el);

    copyChipOnKeyDown(event);

    expect(event.preventDefault).not.toHaveBeenCalled();
    expect(writeText).not.toHaveBeenCalled();
  });
});
