/** "Stamp this results page": mark saved/applied jobs on a search-results page. */

import { getStampResultsPages } from '../lib/appearance';
import type { BridgeClient } from '../lib/bridge';
import type { PopupResponse } from '../lib/messages';
import type { CollectedCard, StampInput } from '../lib/results-stamp';
import { getToken } from '../lib/storage';
import { getClient, notPaired } from './bridge-client';
import { isCollectedCards } from './guards';
import {
  callPageGlobal,
  CAPTURE_FAILED_MSG,
  injectAndRun,
  readableActiveTab,
  UNREADABLE_PAGE_MSG,
} from './page';

/** Isolated-world global keys under which `results-stamp.js` exposes the
 *  collector/stamper. MUST match `RESULTS_COLLECT_GLOBAL`/`RESULTS_STAMP_GLOBAL`
 *  in `lib/results-stamp.ts`. */
const RESULTS_COLLECT_GLOBAL = '__ajhCollectResultsCards';
const RESULTS_STAMP_GLOBAL = '__ajhStampResultsCards';

/** Step one: inject the results-stamp entry (registers BOTH globals) and call
 *  the collector for the candidate `{url, index}[]`. */
async function injectResultsCollect(tabId: number): Promise<CollectedCard[]> {
  const collected = await injectAndRun(tabId, 'results-stamp.js', RESULTS_COLLECT_GLOBAL, []);
  if (!isCollectedCards(collected)) throw new Error('Could not read job cards on this page.');
  return collected;
}

/** Step two: call the SAME injected instance's stamper with the resolved
 *  `applied.check.batch` entries (same order the urls were sent — the stamper
 *  maps them back to its collected anchors by index). Returns the stamped count. */
async function injectResultsStamp(tabId: number, entries: StampInput[]): Promise<number> {
  const stamped = await callPageGlobal(tabId, RESULTS_STAMP_GLOBAL, [entries]);
  return typeof stamped === 'number' ? stamped : 0;
}

const stampStatus = (status: string, stamped = 0): PopupResponse => ({
  ok: true,
  kind: 'stampResults',
  stamped,
  status,
});

/**
 * User-clicked "Stamp this results page". UNLIKE most gesture verbs, a refusal
 * beyond "not paired" / "preference off" / "unreadable page" (over-cap,
 * throttled, a malformed batch reply, an injection failure) degrades to
 * `stamped: 0` + an explanatory `status` rather than `ok:false` — "respect the
 * refusals… degrade to no stamps, never a partial lie". Restricted pages are
 * answered with the shared {@link UNREADABLE_PAGE_MSG}; the reload hint
 * ({@link CAPTURE_FAILED_MSG}) stays reserved for transient failures on
 * readable pages (#1219).
 */
export async function runStampResults(windowId?: number): Promise<PopupResponse> {
  if (!(await getToken())) return notPaired();
  // Re-read the preference here too (never cached at load) — defense in depth
  // against a stale UI that still shows the button after it was turned off.
  if (!(await getStampResultsPages())) {
    return { ok: false, error: 'Results-page stamps are off. Turn them on in Settings.' };
  }

  // Resolve the active tab ONCE: the collect injection, the batch check and
  // the stamp injection must all target the tab this gesture started on.
  const tab = await readableActiveTab(windowId);
  if (!tab) return { ok: false, error: UNREADABLE_PAGE_MSG };
  const { tabId } = tab;

  let collected: CollectedCard[];
  try {
    collected = await injectResultsCollect(tabId);
  } catch {
    // Transient failure on a readable page — the reload hint is truthful here.
    return { ok: false, error: CAPTURE_FAILED_MSG };
  }
  if (collected.length === 0) return stampStatus('No job cards found on this page.');

  let batch: Awaited<ReturnType<BridgeClient['checkAppliedBatch']>>;
  try {
    batch = await getClient().checkAppliedBatch(collected.map((c) => c.url));
  } catch {
    return stampStatus('Could not reach the desktop app.');
  }
  if (!batch.ok) return stampStatus(batch.error);

  const entries: StampInput[] = batch.results.map((r) => {
    const out: StampInput = { url: r.url, found: r.found };
    if (r.status !== undefined) out.status = r.status;
    return out;
  });

  let stamped: number;
  try {
    stamped = await injectResultsStamp(tabId, entries);
  } catch {
    return stampStatus('Could not stamp this page.');
  }

  return stampStatus(
    stamped > 0
      ? `Stamped ${stamped} card${stamped === 1 ? '' : 's'}.`
      : 'No saved/applied jobs found among the cards on this page.',
    stamped
  );
}
