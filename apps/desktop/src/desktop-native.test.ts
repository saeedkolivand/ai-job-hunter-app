/**
 * Unit tests for desktop-native.ts
 *
 * Strategy:
 * - `import.meta.env.DEV` is read both at `installDesktopNativeBehaviors()` call
 *   time (to gate keydown/wheel registration) and inside the contextmenu callback
 *   (to gate preventDefault). We therefore use `vi.stubEnv` + `vi.resetModules()`
 *   + dynamic import to get a fresh module instance per environment scenario.
 * - Because `document` persists across tests in jsdom, we intercept
 *   `document.addEventListener` before each test group and restore + remove all
 *   registered listeners in `afterEach` so production-mode listeners never bleed
 *   into dev-mode assertions.
 * - Events are dispatched from real DOM nodes appended to `document.body` so that
 *   capture-phase listeners on `document` see them.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

// ---------------------------------------------------------------------------
// Listener-tracking helpers
// ---------------------------------------------------------------------------

type ListenerEntry = {
  type: string;
  listener: EventListenerOrEventListenerObject;
  options?: boolean | AddEventListenerOptions;
};

/**
 * Intercepts `document.addEventListener` and returns a cleanup function that
 * removes every listener registered while the intercept is active.
 */
function interceptDocumentListeners(): () => void {
  const registered: ListenerEntry[] = [];
  const original = document.addEventListener.bind(document);

  document.addEventListener = (
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions
  ) => {
    registered.push({ type, listener, options });
    original(type, listener, options);
  };

  return () => {
    // Restore the real method first, then remove every tracked listener.
    document.addEventListener = original;
    for (const { type, listener, options } of registered) {
      document.removeEventListener(type, listener, options);
    }
    registered.length = 0;
  };
}

// ---------------------------------------------------------------------------
// Event-factory helpers
// ---------------------------------------------------------------------------

function appendAndDispatch(el: HTMLElement, event: Event): { defaultPrevented: boolean } {
  document.body.appendChild(el);
  el.dispatchEvent(event);
  document.body.removeChild(el);
  return { defaultPrevented: event.defaultPrevented };
}

function contextmenuEvent(): MouseEvent {
  return new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
}

function keydownEvent(
  key: string,
  modifiers: { ctrlKey?: boolean; metaKey?: boolean; shiftKey?: boolean } = {}
): KeyboardEvent {
  return new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...modifiers });
}

function wheelEvent(modifiers: { ctrlKey?: boolean; metaKey?: boolean } = {}): WheelEvent {
  return new WheelEvent('wheel', { bubbles: true, cancelable: true, ...modifiers });
}

// ---------------------------------------------------------------------------
// isSelectableTarget — pure function, no env dependency
// ---------------------------------------------------------------------------

describe('isSelectableTarget', () => {
  let isSelectableTarget: (target: EventTarget | null) => boolean;

  beforeEach(async () => {
    vi.resetModules();
    const mod = await import('./desktop-native');
    isSelectableTarget = mod.isSelectableTarget;
  });

  it('returns true for an <input> element', () => {
    const el = document.createElement('input');
    expect(isSelectableTarget(el)).toBe(true);
  });

  it('returns true for a <textarea> element', () => {
    const el = document.createElement('textarea');
    expect(isSelectableTarget(el)).toBe(true);
  });

  it('returns true for a [contenteditable] element', () => {
    const el = document.createElement('div');
    el.setAttribute('contenteditable', 'true');
    expect(isSelectableTarget(el)).toBe(true);
  });

  it('returns true for a .select-text element', () => {
    const el = document.createElement('span');
    el.className = 'select-text';
    expect(isSelectableTarget(el)).toBe(true);
  });

  it('returns true for a [data-selectable] element', () => {
    const el = document.createElement('div');
    el.setAttribute('data-selectable', '');
    expect(isSelectableTarget(el)).toBe(true);
  });

  it('returns true for a child nested inside a .select-text container', () => {
    const container = document.createElement('p');
    container.className = 'select-text';
    const child = document.createElement('span');
    container.appendChild(child);
    expect(isSelectableTarget(child)).toBe(true);
  });

  it('returns false for a plain <div>', () => {
    const el = document.createElement('div');
    expect(isSelectableTarget(el)).toBe(false);
  });

  it('returns false for null', () => {
    expect(isSelectableTarget(null)).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// installDesktopNativeBehaviors — production mode
// ---------------------------------------------------------------------------

describe('installDesktopNativeBehaviors (production)', () => {
  let cleanup: () => void;

  beforeEach(async () => {
    cleanup = interceptDocumentListeners();
    vi.resetModules();
    vi.stubEnv('DEV', false);
    const mod = await import('./desktop-native');
    mod.installDesktopNativeBehaviors();
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllEnvs();
  });

  // --- contextmenu ---

  it('prevents contextmenu on a plain div', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(el, contextmenuEvent());
    expect(defaultPrevented).toBe(true);
  });

  it('does NOT prevent contextmenu on a .select-text element', () => {
    const el = document.createElement('span');
    el.className = 'select-text';
    const { defaultPrevented } = appendAndDispatch(el, contextmenuEvent());
    expect(defaultPrevented).toBe(false);
  });

  it('does NOT prevent contextmenu on an <input>', () => {
    const el = document.createElement('input');
    const { defaultPrevented } = appendAndDispatch(el, contextmenuEvent());
    expect(defaultPrevented).toBe(false);
  });

  // --- keydown zoom shortcuts ---

  it('prevents Ctrl+= (zoom in)', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(el, keydownEvent('=', { ctrlKey: true }));
    expect(defaultPrevented).toBe(true);
  });

  it('prevents Ctrl+0 (zoom reset)', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(el, keydownEvent('0', { ctrlKey: true }));
    expect(defaultPrevented).toBe(true);
  });

  it('prevents Ctrl+- (zoom out)', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(el, keydownEvent('-', { ctrlKey: true }));
    expect(defaultPrevented).toBe(true);
  });

  it('prevents Ctrl++ (zoom in alternate)', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(el, keydownEvent('+', { ctrlKey: true }));
    expect(defaultPrevented).toBe(true);
  });

  it('prevents Cmd+= (zoom in, macOS metaKey)', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(el, keydownEvent('=', { metaKey: true }));
    expect(defaultPrevented).toBe(true);
  });

  it('prevents Ctrl+r (reload)', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(el, keydownEvent('r', { ctrlKey: true }));
    expect(defaultPrevented).toBe(true);
  });

  it('prevents Cmd+r (reload, macOS metaKey)', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(el, keydownEvent('r', { metaKey: true }));
    expect(defaultPrevented).toBe(true);
  });

  it('prevents Ctrl+Shift+R (hard reload)', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(
      el,
      keydownEvent('R', { ctrlKey: true, shiftKey: true })
    );
    expect(defaultPrevented).toBe(true);
  });

  it('prevents Cmd+Shift+R (hard reload, macOS metaKey)', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(
      el,
      keydownEvent('R', { metaKey: true, shiftKey: true })
    );
    expect(defaultPrevented).toBe(true);
  });

  it('prevents F5 (reload)', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(el, keydownEvent('F5'));
    expect(defaultPrevented).toBe(true);
  });

  it('does NOT prevent a plain keydown (letter a)', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(el, keydownEvent('a'));
    expect(defaultPrevented).toBe(false);
  });

  it('does NOT prevent Ctrl+s (unrelated shortcut)', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(el, keydownEvent('s', { ctrlKey: true }));
    expect(defaultPrevented).toBe(false);
  });

  // --- wheel zoom ---

  it('prevents wheel with ctrlKey (pinch-to-zoom)', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(el, wheelEvent({ ctrlKey: true }));
    expect(defaultPrevented).toBe(true);
  });

  it('prevents wheel with metaKey (pinch-to-zoom on macOS)', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(el, wheelEvent({ metaKey: true }));
    expect(defaultPrevented).toBe(true);
  });

  it('does NOT prevent wheel without ctrlKey/metaKey', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(el, wheelEvent({}));
    expect(defaultPrevented).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// installDesktopNativeBehaviors — dev mode
// ---------------------------------------------------------------------------

describe('installDesktopNativeBehaviors (dev)', () => {
  let cleanup: () => void;

  beforeEach(async () => {
    cleanup = interceptDocumentListeners();
    vi.resetModules();
    vi.stubEnv('DEV', true);
    const mod = await import('./desktop-native');
    mod.installDesktopNativeBehaviors();
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllEnvs();
  });

  it('does NOT prevent contextmenu on a plain div (Inspect Element preserved)', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(el, contextmenuEvent());
    expect(defaultPrevented).toBe(false);
  });

  it('does NOT prevent contextmenu on a .select-text element in dev', () => {
    const el = document.createElement('span');
    el.className = 'select-text';
    const { defaultPrevented } = appendAndDispatch(el, contextmenuEvent());
    expect(defaultPrevented).toBe(false);
  });

  it('does NOT prevent zoom keydown (Ctrl+=) in dev', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(el, keydownEvent('=', { ctrlKey: true }));
    expect(defaultPrevented).toBe(false);
  });

  it('does NOT prevent reload keydown (F5) in dev', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(el, keydownEvent('F5'));
    expect(defaultPrevented).toBe(false);
  });

  it('does NOT prevent Ctrl+r in dev', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(el, keydownEvent('r', { ctrlKey: true }));
    expect(defaultPrevented).toBe(false);
  });

  it('does NOT prevent wheel with ctrlKey in dev', () => {
    const el = document.createElement('div');
    const { defaultPrevented } = appendAndDispatch(el, wheelEvent({ ctrlKey: true }));
    expect(defaultPrevented).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// SELECTABLE_SELECTOR — constant shape check
// ---------------------------------------------------------------------------

describe('SELECTABLE_SELECTOR', () => {
  it('contains all expected selector fragments', async () => {
    vi.resetModules();
    const { SELECTABLE_SELECTOR } = await import('./desktop-native');
    expect(SELECTABLE_SELECTOR).toContain('input');
    expect(SELECTABLE_SELECTOR).toContain('textarea');
    expect(SELECTABLE_SELECTOR).toContain('[contenteditable]');
    expect(SELECTABLE_SELECTOR).toContain('.select-text');
    expect(SELECTABLE_SELECTOR).toContain('[data-selectable]');
  });
});
