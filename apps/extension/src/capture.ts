/**
 * Answers-capture injected entry (compiled to `capture.js`).
 *
 * Injected via `chrome.scripting.executeScript({ files: ['capture.js'] })` on
 * the user's "Save my answers from this page" click — a single-step classic
 * script (mirrors `content.ts`'s pattern, not `fill.ts`'s two-step
 * register+invoke): the collector takes no PII to pass in transiently, it
 * only reads the page and returns data, so there is nothing to keep off a
 * stored/registered surface.
 *
 * Returns BOTH `answers` (for `answers.save`, unchanged) AND `filled` (PR 11:
 * the rewrite-mode picker's scan-time correlation, via
 * {@link collectFilledFields}) from this SAME injection — no separate scan
 * trigger for rewrite mode, mirroring how `answers.suggest`'s existing scan
 * already doubles as the draft-mode picker's source.
 *
 * Bundled with ZERO `import` statements: `./lib/answers-capture` (and the
 * `./lib/field-signal` helpers it shares with `autofill.ts`) is inlined here
 * because this file is built by its OWN isolated Rollup pass — see the
 * `injectedEntries` plugin in `vite.config.ts` — so it never shares a chunk
 * with `fill.js`.
 */

import { collectAnswers, collectFilledFields } from './lib/answers-capture';

// ── injected-execution entry-point ────────────────────────────────────────────
// Completion value returned to executeScript → background (mirrors content.ts:
// the IIFE's return value is the last statement's value, which is what
// `executeScript` hands back).
(() => ({ answers: collectAnswers(document), filled: collectFilledFields(document) }))();
