/**
 * The Notebook identity's hex/rgba values, as plain JSON-safe string
 * constants — so they can cross the `executeScript` args boundary (PR3 §B.2;
 * see the PR2 lesson repeated in the spec: only plain primitives survive
 * that boundary, never a class instance or a `CSSStyleDeclaration`).
 *
 * Mirrors the custom properties in `popup/popup.css`'s base `:root` block
 * (light) and its `@media (prefers-color-scheme: dark)` / `:root[data-theme
 * ='dark']` blocks (dark) — kept in lockstep by `notebook-palette.test.ts`'s
 * parity test, which reads `popup.css` and asserts every value here matches
 * the corresponding token in BOTH the light and the dark source, so an
 * injected badge/stamp can never silently drift from the extension's own
 * identity.
 *
 * A page-injected script cannot read the extension's own CSS custom
 * properties (they live in a different document) — this module is the
 * portable copy an injected entry inlines instead.
 */

export interface NotebookPalette {
  paper: string;
  card: string;
  ink: string;
  inkSoft: string;
  red: string;
  redInk: string;
  ok: string;
  warn: string;
  shadow: string;
}

/** Mirrors `popup.css`'s base `:root` block. */
export const NOTEBOOK_LIGHT: NotebookPalette = {
  paper: '#f4ecdc',
  card: '#fffdf6',
  ink: '#1c1812',
  inkSoft: '#4a4332',
  red: '#e24b4a',
  redInk: '#b5302f',
  ok: '#2f7d4f',
  warn: '#9a6a12',
  shadow: 'rgba(28, 24, 18, 0.16)',
};

/** Mirrors `popup.css`'s `@media (prefers-color-scheme: dark)` AND
 *  `:root[data-theme='dark']` blocks (the two are byte-identical there). */
export const NOTEBOOK_DARK: NotebookPalette = {
  paper: '#1a1612',
  card: '#241f19',
  ink: '#efe6d4',
  inkSoft: '#b9ac95',
  red: '#ef5d5a',
  redInk: '#ef5d5a',
  ok: '#5fd08a',
  warn: '#e0b35a',
  shadow: 'rgba(0, 0, 0, 0.45)',
};

/** Pure selection — no `matchMedia`, no DOM. Exported so tests exercise the
 *  branch directly. */
export function pickNotebookPalette(prefersDark: boolean): NotebookPalette {
  return prefersDark ? NOTEBOOK_DARK : NOTEBOOK_LIGHT;
}

/**
 * Read the PAGE's own color-scheme preference and pick a palette set.
 * Call only from INSIDE the injected script (the page's own `window`, never
 * the background's) — the badge/stamp render into the page's isolated
 * world, which may have a different effective color scheme than the
 * extension's own popup/panel.
 */
export function currentNotebookPalette(win: Window): NotebookPalette {
  return pickNotebookPalette(win.matchMedia('(prefers-color-scheme: dark)').matches);
}
