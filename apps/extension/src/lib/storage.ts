/**
 * Pairing-token persistence in `chrome.storage.local` (via @wxt-dev/browser
 * so the same code runs on Chrome + Firefox). The token is a 64-char hex secret
 * the user copies from the desktop app's Settings → "Browser extension".
 *
 * Storage scope: this never leaves the extension. The token is only ever sent
 * back to the loopback desktop bridge, never to any remote server.
 */

import { browser } from '@wxt-dev/browser';

const TOKEN_KEY = 'pairingToken';

/** The desktop token is 32 random bytes as lowercase hex → 64 chars. */
const TOKEN_HEX_LENGTH = 64;

/** Read the stored pairing token, or `null` if not paired. */
export async function getToken(): Promise<string | null> {
  const stored = await browser.storage.local.get(TOKEN_KEY);
  const value = stored[TOKEN_KEY];
  return typeof value === 'string' && value.length > 0 ? value : null;
}

/** Persist a pasted token. Returns the trimmed value actually stored. */
export async function setToken(token: string): Promise<string> {
  const trimmed = token.trim();
  await browser.storage.local.set({ [TOKEN_KEY]: trimmed });
  return trimmed;
}

/** Remove the stored token (un-pair). */
export async function clearToken(): Promise<void> {
  await browser.storage.local.remove(TOKEN_KEY);
}

/**
 * Cheap shape check for a pasted token: lowercase hex of the expected length.
 * The desktop is the real authority (it rejects a bad token per-frame); this
 * only catches obvious paste mistakes before we store one.
 */
export function looksLikeToken(value: string): boolean {
  return new RegExp(`^[0-9a-f]{${TOKEN_HEX_LENGTH}}$`).test(value.trim());
}
