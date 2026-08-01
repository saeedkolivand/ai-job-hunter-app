// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { OnFn } from './geometry';
import { ARROW_STEP, attachKeyboardShortcuts } from './keyboard';
import type { ViewportControls } from './viewport';

type MockViewport = Pick<ViewportControls, 'fit' | 'zoomAt' | 'panBy' | 'stageCenter'>;

function mockViewport(): MockViewport {
  return {
    fit: vi.fn(),
    zoomAt: vi.fn(),
    panBy: vi.fn(),
    stageCenter: vi.fn((): [number, number] => [0, 0]),
  };
}

// Bare-bones `on` collector, same shape interactions.ts wires up for real —
// keeps every listener this file attaches removable between tests.
let cleanups: Array<() => void> = [];
const on: OnFn = (target, type, handler, opts) => {
  target.addEventListener(type, handler, opts);
  cleanups.push(() => target.removeEventListener(type, handler, opts));
};

afterEach(() => {
  cleanups.forEach((fn) => fn());
  cleanups = [];
  document.body.innerHTML = '';
});

function setup() {
  const root = document.createElement('div');
  root.innerHTML = '<aside id="side" tabindex="0"></aside>';
  // root must be attached so a keydown fired on #side actually bubbles up to
  // the window-level listener under test (detached trees stop bubbling at
  // their own top, never reaching window).
  document.body.appendChild(root);
  const sideEl = root.querySelector<HTMLElement>('#side');
  if (!sideEl) throw new Error('fixture missing #side');
  const viewport = mockViewport();
  const onEscape = vi.fn();
  attachKeyboardShortcuts(root, on, { viewport, onEscape });
  return { sideEl, viewport, onEscape };
}

function dispatchKey(target: EventTarget, key: string): KeyboardEvent {
  const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
  target.dispatchEvent(event);
  return event;
}

describe('attachKeyboardShortcuts — sidebar arrow-key exclusion (#side)', () => {
  it('pans the map and prevents default on ArrowDown when focus is outside the sidebar', () => {
    const { viewport } = setup();
    const event = dispatchKey(window, 'ArrowDown');
    expect(viewport.panBy).toHaveBeenCalledWith(0, -ARROW_STEP);
    expect(event.defaultPrevented).toBe(true);
  });

  it.each(['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'] as const)(
    'leaves %s un-prevented and does not pan when the focused sidebar is the event target',
    (key) => {
      const { sideEl, viewport } = setup();
      const event = dispatchKey(sideEl, key);
      expect(viewport.panBy).not.toHaveBeenCalled();
      // preventDefault() must NOT fire — the browser's native scroll on the
      // focused, overflow:auto sidebar is what makes it keyboard-scrollable.
      expect(event.defaultPrevented).toBe(false);
    }
  );

  it('still clears the selection on Escape while focus is inside the sidebar', () => {
    const { sideEl, onEscape } = setup();
    dispatchKey(sideEl, 'Escape');
    expect(onEscape).toHaveBeenCalledTimes(1);
  });

  it('still fits the view on "f" while focus is inside the sidebar', () => {
    const { sideEl, viewport } = setup();
    dispatchKey(sideEl, 'f');
    expect(viewport.fit).toHaveBeenCalledTimes(1);
  });
});
