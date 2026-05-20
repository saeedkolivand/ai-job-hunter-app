/**
 * Board-specific auth configurations.
 *
 * Each board gets a dedicated `persist:<id>` Electron partition.
 * Chromium stores all cookies, localStorage, and IndexedDB for that partition
 * under <userData>/Partitions/persist_<id>/ automatically.
 * The user logs in once; the session survives app restarts indefinitely
 * (or until the user explicitly disconnects).
 *
 * ── Auth detection notes ────────────────────────────────────────────────────
 *
 * LinkedIn:
 *   Full-page navigation after login → li_at cookie appears.
 *   Cookie-based detection is reliable and instant.
 *   Passkey prompts blocked via navigator.credentials override.
 *
 * Indeed:
 *   AJAX login → auth cookies set WITHOUT a page navigation.
 *   We watch session.cookies.on('changed') for a new cookie from
 *   secure.indeed.com with a value longer than 40 chars (auth tokens are
 *   always long; cookie-banner consent values are short: "true", timestamps).
 *   Baseline snapshot (captured on did-finish-load) excludes the 7+ tracking
 *   cookies Indeed sets via HTTP redirect headers before the page renders.
 *
 * Xing / Glassdoor:
 *   URL-based: once the browser leaves the auth subdomain the user is in.
 */

import type { BoardConfig } from './types.js';

/** Passkey script injected into the page's main world on every dom-ready. */
export const DISABLE_PASSKEY_SCRIPT = `
(function () {
  try {
    const orig = navigator.credentials;
    if (!orig) return;
    const origGet = orig.get.bind(orig);
    const origCreate = orig.create.bind(orig);
    const notAllowed = () =>
      Promise.reject(
        Object.assign(new DOMException('User cancelled', 'NotAllowedError'), { code: 20 })
      );
    Object.defineProperty(navigator, 'credentials', {
      configurable: true,
      get: () => ({
        get:  (o) => (o?.publicKey ? notAllowed() : origGet(o)),
        create: (o) => (o?.publicKey ? notAllowed() : origCreate(o)),
        store: orig.store.bind(orig),
        preventSilentAccess: orig.preventSilentAccess.bind(orig),
      }),
    });
  } catch (_) {}
})();
`.trim();

export const BOARD_CONFIGS: Record<string, BoardConfig> = {
  linkedin: {
    id: 'linkedin',
    displayName: 'LinkedIn',
    loginUrl: 'https://www.linkedin.com/login',
    partition: 'persist:linkedin',
    blockPasskeys: true,
    // li_at is LinkedIn's canonical session cookie.
    // It is never present before login; baseline is ignored.
    isAuthenticatedCookies: (cookies) => {
      const liAt = cookies.find((c) => c.name === 'li_at' && c.domain?.includes('linkedin.com'));
      return !!liAt && liAt.value.length > 10;
    },
  },

  indeed: {
    id: 'indeed',
    displayName: 'Indeed',
    loginUrl: 'https://secure.indeed.com/auth',
    partition: 'persist:indeed',
    // Auth detection: wait for baseline, then look for a new cookie from
    // secure.indeed.com with a long value (auth token, not consent flag).
    // Cookie-banner accept sets short-valued cookies ("true", timestamps)
    // on consent subdomains — those are excluded by the value.length check.
    isAuthenticatedCookies: (cookies, baseline) => {
      if (!baseline) return false;
      const seen = new Set(baseline.map((c) => `${c.name}\x00${c.domain}`));
      return cookies.some(
        (c) =>
          c.domain?.includes('indeed.com') &&
          !seen.has(`${c.name}\x00${c.domain}`) &&
          c.value.length > 40 // auth tokens >> consent flag values
      );
    },
  },

  xing: {
    id: 'xing',
    displayName: 'Xing',
    loginUrl: 'https://login.xing.com/login',
    partition: 'persist:xing',
    isAuthenticatedUrl: (u) =>
      u.includes('xing.com') && !u.includes('login.xing.com') && !u.includes('/login'),
  },

  glassdoor: {
    id: 'glassdoor',
    displayName: 'Glassdoor',
    loginUrl: 'https://www.glassdoor.com/profile/login_input.htm',
    partition: 'persist:glassdoor',
    isAuthenticatedUrl: (u) =>
      u.includes('glassdoor.com') && !u.includes('/profile/login') && !u.includes('/index.htm?sso'),
  },
};
