/**
 * Results-page stamps injected entry (compiled to `results-stamp.js`, PR3).
 *
 * Injected once via `chrome.scripting.executeScript({ files: [
 * 'results-stamp.js'] })`, then called TWICE by the background via a
 * second/third `executeScript({ func })`: first {@link RESULTS_COLLECT_GLOBAL}
 * (no args — returns the candidate `{url, index}[]`), then, after the
 * `applied.check.batch` round trip,
 * {@link RESULTS_STAMP_GLOBAL} (one JSON-safe results array — see the PR2
 * lesson). Mirrors `fill.ts`'s two-step register-then-invoke pattern, with
 * one extra invoke since this feature has two distinct steps to answer.
 */

import {
  RESULTS_COLLECT_GLOBAL,
  RESULTS_STAMP_GLOBAL,
  runCollectResultsCards,
  runStampResultsCards,
} from './lib/results-stamp';

const globals = globalThis as unknown as Record<string, unknown>;
globals[RESULTS_COLLECT_GLOBAL] = runCollectResultsCards;
globals[RESULTS_STAMP_GLOBAL] = runStampResultsCards;

// Ensure this file is treated as an ES module (see fill.ts's identical note).
export {};
