/**
 * Assisted-autofill injected entry (compiled to `fill.js`).
 *
 * Injected on the user's click via `chrome.scripting.executeScript({ files:
 * ['fill.js'] })` — NOT a persistently registered content script (`activeTab` +
 * `scripting`, scoped to the clicked tab, same pattern as Scan-mode `content.ts`).
 *
 * It only exposes {@link runAutofill} on the page's isolated-world global; the
 * background then calls it with the contact profile via a second
 * `executeScript({ func, args })`. Splitting the data-passing out of `files`
 * injection keeps the PII (the profile) out of any stored/registered surface — it
 * is handed in transiently for the one call and never persisted.
 */

import {
  AUTOFILL_GLOBAL,
  type AutofillProfile,
  type AutofillSummary,
  runAutofill,
} from './lib/autofill';

// Expose the filler on the isolated-world global under a namespaced key so the
// background's second `executeScript({ func })` can invoke it with the profile.
(globalThis as unknown as Record<string, (p: AutofillProfile) => AutofillSummary>)[
  AUTOFILL_GLOBAL
] = runAutofill;

// Ensure this file is treated as an ES module.
export {};
