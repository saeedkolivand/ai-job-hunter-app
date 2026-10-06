/** "Check fit": score the active page against the user's résumé, then paint the on-page badge. */

import { browser } from '@wxt-dev/browser';

import type { ExtensionMatchLiveRequest, ExtensionMatchLiveResult } from '@ajh/shared';

import { getShowFitBadge } from '../lib/appearance';
import type { FitBadgeView } from '../lib/fit-badge';
import type { PopupResponse } from '../lib/messages';
import { getToken } from '../lib/storage';
import { getClient, notPaired } from './bridge-client';
import {
  CAPTURE_FAILED_MSG,
  captureTabHtml,
  injectFile,
  readableActiveTab,
  tabStillOnExactUrl,
  UNREADABLE_PAGE_MSG,
} from './page';

/** Isolated-world global key under which `fit-badge.js` exposes the renderer.
 *  MUST match `FIT_BADGE_GLOBAL` in `lib/fit-badge.ts`. */
const FIT_BADGE_GLOBAL = '__ajhRenderFitBadge';

/** Qualitative band next to the score — mirrors `job-tools.ts::scoreBand`
 *  EXACTLY (duplicated rather than imported: `job-tools.ts` is UI-mounting code
 *  with its own dependency chain, and this is a 3-line pure function, not a
 *  wire contract). */
function fitBadgeScoreBand(score: number): FitBadgeView['band'] {
  if (score >= 80) return 'strong match';
  if (score >= 50) return 'partial match';
  return 'low match';
}

/** Mirrors `job-tools.ts`'s `SCORE_SOURCE_LABEL` EXACTLY (same duplication
 *  discipline): the on-page badge must show the score's qualifier too, never
 *  one tap deeper than the panel/popup card does. */
const FIT_BADGE_SCORE_SOURCE_LABEL: Record<'keyword' | 'combined', string> = {
  keyword: 'keyword coverage',
  combined: 'combined (keyword + semantic)',
};

/**
 * Inject the on-page fit badge into `tabId` and render `view`. Two steps so the
 * match result is passed transiently as an `executeScript` arg (every value on
 * `view` is JSON-safe).
 *
 * `url` is the SAME url `runMatchLive` captured before the desktop round trip
 * (and `tabStillOnExactUrl` re-verified against `tabs.url` just before this
 * call). It is re-checked ONE more time, IN the page, immediately before the
 * renderer runs — the last possible point, catching a navigation during
 * `maybeShowFitBadge`'s OWN later awaits (`getShowFitBadge`, `checkApplied`),
 * which land after the background-side check. A full navigation loads a fresh
 * document that this call re-injects into; an SPA navigation keeps the
 * installed global alive on the SAME document with a new `location.href`.
 * Either way `location.href` is the page's own live truth, so an EXACT match is
 * the only comparison that can never let a different posting through.
 *
 * The badge keeps verifying after render (see `lib/fit-badge.ts`'s
 * `renderFitBadge`): the captured `expectedUrl` is what its watcher compares
 * against, so this seam stays the single source of truth for "which posting is
 * the badge about" (#1221).
 */
async function injectFitBadge(tabId: number, url: string, view: FitBadgeView): Promise<void> {
  await injectFile(tabId, 'fit-badge.js');
  await browser.scripting.executeScript({
    target: { tabId },
    func: (v: FitBadgeView, key: string, expectedUrl: string): void => {
      if (location.href !== expectedUrl) return;
      const runner = (globalThis as Record<string, unknown>)[key] as
        ((view: FitBadgeView, expectedUrl?: string) => void) | undefined;
      runner?.(v, expectedUrl);
    },
    args: [view, FIT_BADGE_GLOBAL, url],
  });
}

/**
 * Best-effort on-page badge after a successful Check-fit — ONLY when
 * `getShowFitBadge()` is true. Every failure here (opt-in read, the
 * applied-status lookup, the injection itself) is swallowed: this is a UI
 * enhancement on an already-successful gesture, never its own result.
 */
async function maybeShowFitBadge(
  tabId: number,
  url: string,
  result: Extract<ExtensionMatchLiveResult, { ok: true }>
): Promise<void> {
  try {
    if (!(await getShowFitBadge())) return;
    const score = Math.round(result.combined);

    let applied: FitBadgeView['applied'] = null;
    try {
      const check = await getClient().checkApplied(url);
      if (check.found) applied = check.status === 'applied' ? 'applied' : 'saved';
    } catch {
      // Best-effort — the badge still renders without the saved/applied chip.
    }

    const view: FitBadgeView = {
      score,
      band: fitBadgeScoreBand(score),
      scoreLabel: FIT_BADGE_SCORE_SOURCE_LABEL[result.scoreSource],
      gaps: result.gaps,
      applied,
    };
    if (result.salary) view.salary = result.salary;

    await injectFitBadge(tabId, url, view);
  } catch {
    // Never let a badge-rendering failure surface anywhere.
  }
}

/**
 * User-clicked "Check fit". A deliberate click, so failures propagate to the
 * dispatcher's outer catch. UNLIKE `runImport` there is no URL-only fallback:
 * `match.live` requires the captured DOM, so a capture failure surfaces as a
 * user-facing error. Restricted pages are caught EARLY and answered with
 * {@link UNREADABLE_PAGE_MSG}; the transient {@link CAPTURE_FAILED_MSG} reload
 * hint is reserved for capture failures on pages that genuinely CAN be read
 * (#1219).
 */
export async function runMatchLive(windowId?: number): Promise<PopupResponse> {
  if (!(await getToken())) return notPaired();

  // Resolve the tab identity ONCE — the url, the html capture, and (after the
  // round trip below) the badge injection must all target the SAME tab (PR
  // review finding: a tab switch mid-request could otherwise paint one page's
  // score onto a different page).
  const tab = await readableActiveTab(windowId);
  if (!tab) return { ok: false, error: UNREADABLE_PAGE_MSG };
  const { tabId, url } = tab;
  let html: string;
  try {
    html = await captureTabHtml(tabId);
  } catch {
    return { ok: false, error: CAPTURE_FAILED_MSG };
  }

  const payload: ExtensionMatchLiveRequest = { url, html };
  const result = await getClient().matchLive(payload);
  if (result.ok) {
    // Fire-and-forget: never something the popup's own response waits on.
    // Re-verify the SAME tab still has the SAME url right before injecting — a
    // tab switch or same-tab navigation during the (possibly slow) desktop
    // round trip must abort the badge silently rather than mis-paint it.
    void (async () => {
      try {
        if (!(await tabStillOnExactUrl(tabId, url, windowId))) return;
        await maybeShowFitBadge(tabId, url, result);
      } catch {
        // No active tab to render into — skip silently.
      }
    })();
  }
  return { ok: true, kind: 'matchLive', result };
}
