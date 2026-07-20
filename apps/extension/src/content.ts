/**
 * Scan-mode DOM capture, injected on demand via `chrome.scripting.executeScript`
 * ({ files: ['content.js'] }). It is NOT a persistently registered content
 * script — `activeTab` + `scripting` inject it only on the user's click, scoped
 * to the clicked tab.
 *
 * The script's completion value is what `executeScript` returns to the
 * background. v1 returns the FULL `document.documentElement.outerHTML` — that is
 * the reliable capture for authenticated pages the desktop's headless fetch
 * can't reach. We additionally annotate a best-effort "main job node" hint by
 * tagging it, but always hand back the whole document so the desktop parser has
 * everything (it does its own extraction).
 *
 * Pure helpers are exported for unit tests; the IIFE at the bottom runs them
 * automatically when the script is injected by executeScript (the injected-
 * execution contract is preserved — the IIFE's completion value is still the
 * outerHTML string that executeScript returns to the background).
 *
 * Bundled with an `import` from `./lib/field-signal` (unlike most injected
 * classic-script entries, this is SAFE for a `chrome.scripting.executeScript`
 * target): built by its OWN isolated Rollup pass — see the `injectedEntries`
 * plugin in `vite.config.ts` — mirroring `capture.ts`'s convention, so the
 * import is inlined and never shares a chunk with another entry.
 */

import { isHidden } from './lib/field-signal';

/**
 * CSS selector priority list for best-effort job-container detection. The
 * detail-pane selectors are tried BEFORE `main` — on a search/list-shell view
 * (e.g. LinkedIn's `?currentJobId=` split-pane) `main` wraps BOTH the list and
 * the selected job's detail pane, so it would mark the whole shell instead of
 * the pane that actually renders the selected job's full description.
 */
const JOB_NODE_CANDIDATES = [
  '[class*="job-details" i]',
  '[class*="jobs-details" i]',
  '[class*="jobs-description" i]',
  'main',
  '[role="main"]',
  'article',
  '[class*="job" i]',
  '[id*="job" i]',
  '[class*="posting" i]',
] as const;

/**
 * Best-effort: find the most likely main job container so a future desktop
 * parser could prefer it. We do not trim to it (full outerHTML is the v1
 * contract); we only mark it so the markup is preserved verbatim. This
 * capture can run on ANY active tab (not just known job boards), so a hidden
 * decoy container is skipped rather than allowed to steal the hint from a
 * real, visible container — `display:none` does NOT inherit, so a candidate
 * can compute its OWN `display` as visible while sitting under a hidden
 * ancestor; `field-signal.ts`'s `isHidden` walks the ancestor chain (and is
 * `getComputedStyle`-only — never `getBoundingClientRect`/`offsetWidth`,
 * which jsdom always zeroes regardless of real visibility), so reusing it
 * here catches that case too.
 *
 * Exported so tests can call the real implementation directly.
 */
export function markLikelyJobNode(): void {
  for (const selector of JOB_NODE_CANDIDATES) {
    for (const el of Array.from(document.querySelectorAll<HTMLElement>(selector))) {
      if (isHidden(el)) continue;
      if (el.textContent && el.textContent.trim().length > 200) {
        el.setAttribute('data-ajh-job-root', 'true');
        return;
      }
    }
  }
}

/**
 * Return the full serialised DOM — the completion value executeScript hands
 * back to the background.
 *
 * Exported so tests can assert the capture contract without re-implementing it.
 */
export function capture(): string {
  return document.documentElement.outerHTML;
}

// ── injected-execution entry-point ────────────────────────────────────────────
// When executeScript injects this file the IIFE runs immediately; its return
// value is the completion value that executeScript passes back to the background.
// Named exports above are invisible to the injected-script runtime but allow
// the test suite to import and call the real functions directly.
(() => {
  try {
    markLikelyJobNode();
  } catch {
    // Marking is advisory; never let it block the capture.
  }

  // Completion value returned to executeScript → background.
  return capture();
})();
