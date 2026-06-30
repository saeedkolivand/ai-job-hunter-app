/**
 * Native-desktop behaviors for the Tauri shell.
 *
 * Makes the renderer feel like a native app rather than a browser tab:
 *  - suppresses the right-click context menu (except over real text content),
 *  - blocks browser zoom (Ctrl/Cmd +/-/=/0 and Ctrl/Cmd + wheel),
 *  - blocks page reload (Ctrl/Cmd+R, Ctrl/Cmd+Shift+R, F5).
 *
 * Every guard is gated behind `import.meta.env.DEV` so development keeps
 * Inspect Element, hot-reload, and zoom working — these are *production-only*.
 *
 * Call {@link installDesktopNativeBehaviors} once before the first render.
 */

/**
 * Allow-list of elements where text selection / the native context menu stay
 * enabled. Kept in sync with the CSS opt-in list in
 * `renderer/styles/globals.css`. The CSS expands descendants explicitly
 * (`.select-text *`); here we walk ancestors with `.closest()` instead — the
 * two are equivalent but maintained separately, so edit both together.
 */
export const SELECTABLE_SELECTOR =
  'input, textarea, [contenteditable], .select-text, [data-selectable]';

/** True when the event target is inside a selectable region. */
export function isSelectableTarget(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest(SELECTABLE_SELECTOR) !== null;
}

/** Keys that trigger browser zoom when combined with Ctrl/Cmd. */
const ZOOM_KEYS = new Set(['+', '-', '=', '0']);

function isZoomShortcut(e: KeyboardEvent): boolean {
  return (e.ctrlKey || e.metaKey) && ZOOM_KEYS.has(e.key);
}

function isReloadShortcut(e: KeyboardEvent): boolean {
  if (e.key === 'F5') return true;
  // Ctrl/Cmd+R and Ctrl/Cmd+Shift+R.
  return (e.ctrlKey || e.metaKey) && (e.key === 'r' || e.key === 'R');
}

/**
 * Install the native-desktop behaviors. Idempotent enough for a single call at
 * startup; listeners live for the lifetime of the app and are never removed.
 */
export function installDesktopNativeBehaviors(): void {
  // Context-menu suppression (production only — dev keeps Inspect Element).
  document.addEventListener(
    'contextmenu',
    (e) => {
      if (import.meta.env.DEV) return;
      if (!isSelectableTarget(e.target)) e.preventDefault();
    },
    { capture: true }
  );

  // Zoom + reload are blocked only in production builds.
  if (import.meta.env.DEV) return;

  document.addEventListener(
    'keydown',
    (e) => {
      if (isZoomShortcut(e) || isReloadShortcut(e)) e.preventDefault();
    },
    { capture: true }
  );

  document.addEventListener(
    'wheel',
    (e) => {
      if (e.ctrlKey || e.metaKey) e.preventDefault();
    },
    { capture: true, passive: false }
  );
}
