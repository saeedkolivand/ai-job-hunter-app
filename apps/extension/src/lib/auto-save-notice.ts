/**
 * The one-shot "here's what the save-answers-on-submit auto-save just did"
 * notice (PR4, decision 7 — "the user must never discover this silently").
 *
 * Lives in `storage.session` (survives the MV3 service-worker idling out,
 * dies with the browser session — same discipline as `lib/answer-state.ts`,
 * NOT `storage.local`: this is page-submit-derived text, not a UI
 * preference). Written once by `background.ts`'s auto-save flow right after
 * a successful `answers.save{auto:true}`; READ-ONCE by whichever surface
 * (popup or panel) asks first via `takeAutoSaveNotice` — the point of a
 * transparent notice is that the user sees it, not that every surface
 * repeats it.
 */

import { type Browser, browser } from '@wxt-dev/browser';

const NOTICE_KEY = 'autoSaveNotice';

/** Mirrors `lib/answer-state.ts`'s own `sessionArea` — a missing session area
 *  (older engine, no-session context) must cost the notice, never a render. */
function sessionArea(): Browser.storage.StorageArea | null {
  const area = (browser.storage as { session?: Browser.storage.StorageArea }).session;
  return area ?? null;
}

/** Record the notice text — called once, right after a successful auto-save. */
export async function setAutoSaveNotice(text: string): Promise<void> {
  const area = sessionArea();
  if (!area) return;
  try {
    await area.set({ [NOTICE_KEY]: text });
  } catch {
    // Best-effort — same discipline as answer-state.ts's writeAnswerState.
  }
}

/** Read (and clear) the pending notice, or `null` when there is none. */
export async function takeAutoSaveNotice(): Promise<string | null> {
  const area = sessionArea();
  if (!area) return null;
  try {
    const stored = await area.get(NOTICE_KEY);
    const value = stored[NOTICE_KEY];
    if (typeof value !== 'string' || value.length === 0) return null;
    await area.remove(NOTICE_KEY).catch(() => undefined);
    return value;
  } catch {
    return null;
  }
}
