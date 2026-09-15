/**
 * On-page fit-badge injected entry (compiled to `fit-badge.js`, PR3).
 *
 * Injected via `chrome.scripting.executeScript({ files: ['fit-badge.js'] })`
 * — NOT a persistently registered content script, scoped to the tab that
 * just ran a successful Check-fit. Mirrors `fill.ts`'s two-step pattern
 * exactly: this file only exposes {@link runRenderFitBadge} on the page's
 * isolated-world global; the background then calls it with the (JSON-safe,
 * plain-object) match view via a second `executeScript({ func, args })`.
 */

import { FIT_BADGE_GLOBAL, type FitBadgeView, runRenderFitBadge } from './lib/fit-badge';

(globalThis as unknown as Record<string, (v: FitBadgeView) => void>)[FIT_BADGE_GLOBAL] =
  runRenderFitBadge;

// Ensure this file is treated as an ES module (see fill.ts's identical note).
export {};
