/**
 * The Appearance section's non-theme preferences (PR0 §5), all
 * `browser.storage.local` (UI preferences, not PII/job data — same
 * discipline as `lib/theme.ts` and `lib/storage.ts`'s Answer-tools flag).
 * `defaultPanelTab` is read only by the options page in THIS PR (the side
 * panel keeps its own per-WINDOW last-active-tab memory —
 * `sidepanel.ts`'s `storage.session` logic — which is a different, narrower
 * feature); the two booleans are written here and consumed by PR3.
 */

import { browser } from '@wxt-dev/browser';

const DEFAULT_TAB_KEY = 'defaultPanelTab';
const SHOW_FIT_BADGE_KEY = 'showFitBadge';
const STAMP_RESULTS_KEY = 'stampResultsPages';

export type DefaultPanelTab = 'job' | 'answers';

/** Defaults to `job` — the same default the side panel's own tab bar starts on. */
export async function getDefaultPanelTab(): Promise<DefaultPanelTab> {
  const stored = await browser.storage.local.get(DEFAULT_TAB_KEY);
  return stored[DEFAULT_TAB_KEY] === 'answers' ? 'answers' : 'job';
}

export async function setDefaultPanelTab(tab: DefaultPanelTab): Promise<void> {
  await browser.storage.local.set({ [DEFAULT_TAB_KEY]: tab });
}

/** Defaults OFF — an opt-in, like every other on-page behavior this extension adds. */
export async function getShowFitBadge(): Promise<boolean> {
  const stored = await browser.storage.local.get(SHOW_FIT_BADGE_KEY);
  return stored[SHOW_FIT_BADGE_KEY] === true;
}

export async function setShowFitBadge(value: boolean): Promise<void> {
  await browser.storage.local.set({ [SHOW_FIT_BADGE_KEY]: value });
}

/** Defaults OFF — same opt-in discipline as {@link getShowFitBadge}. */
export async function getStampResultsPages(): Promise<boolean> {
  const stored = await browser.storage.local.get(STAMP_RESULTS_KEY);
  return stored[STAMP_RESULTS_KEY] === true;
}

export async function setStampResultsPages(value: boolean): Promise<void> {
  await browser.storage.local.set({ [STAMP_RESULTS_KEY]: value });
}
