/**
 * The Settings → Appearance → Theme choice (PR0 §5): System (the attribute's
 * ABSENCE — `popup.css`'s `prefers-color-scheme` media query decides) / Light
 * / Dark (force one or the other via `:root[data-theme]`, which outranks the
 * plain `:root` the media query uses on specificity). Persisted in
 * `browser.storage.local` (a UI preference, not PII/job data — same
 * discipline as `lib/storage.ts`'s Answer-tools expand/collapse flag) and
 * applied by EVERY surface that loads `popup.css`: the popup, the side panel
 * and the options page each call {@link bootTheme} once at load.
 */

import { browser } from '@wxt-dev/browser';

export type Theme = 'system' | 'light' | 'dark';

const THEME_KEY = 'theme';

/** Read the persisted choice, defaulting to `system` for an absent/malformed value. */
export async function getTheme(): Promise<Theme> {
  const stored = await browser.storage.local.get(THEME_KEY);
  const value = stored[THEME_KEY];
  return value === 'light' || value === 'dark' ? value : 'system';
}

/** Set `document.documentElement`'s `data-theme` attribute — `system` removes it. */
export function applyTheme(theme: Theme): void {
  if (theme === 'system') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = theme;
}

/** Persist `theme` and apply it to THIS document immediately. */
export async function setTheme(theme: Theme): Promise<void> {
  if (theme === 'system') await browser.storage.local.remove(THEME_KEY);
  else await browser.storage.local.set({ [THEME_KEY]: theme });
  applyTheme(theme);
}

/**
 * Boot-time hook: apply the persisted theme choice. Best-effort — a storage
 * read failure just leaves the system default (`prefers-color-scheme`)
 * standing, never throws into the caller's own module-load sequence.
 */
export async function bootTheme(): Promise<void> {
  try {
    applyTheme(await getTheme());
  } catch {
    // Best-effort — system default stands.
  }
}
