import type { KeyboardEvent as ReactKeyboardEvent, MouseEvent as ReactMouseEvent } from 'react';

import { COPIED_TIMEOUT_MS } from './constants';

/** Click-to-copy: copy the chip's data-copy, briefly flag it "copied". */
export function copyChip(event: ReactMouseEvent<HTMLElement>): void {
  const el = event.currentTarget;
  const text = el.getAttribute('data-copy') ?? el.textContent ?? '';
  if (!navigator.clipboard) return;
  void navigator.clipboard.writeText(text).then(() => {
    el.classList.add('copied');
    window.setTimeout(() => el.classList.remove('copied'), COPIED_TIMEOUT_MS);
  });
}

/** Shared Enter/Space activation for a `role="button"` `.copy-cmd` chip. */
export function copyChipOnKeyDown(event: ReactKeyboardEvent<HTMLElement>): void {
  if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault();
    copyChip(event as unknown as ReactMouseEvent<HTMLElement>);
  }
}
