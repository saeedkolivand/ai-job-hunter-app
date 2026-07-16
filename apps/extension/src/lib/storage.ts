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

/** Popup UI preference — NOT PII/job data, just whether the "Answer tools"
 *  disclosure should render pre-expanded. */
const ANSWER_TOOLS_EXPANDED_KEY = 'answerToolsExpanded';

/** The desktop token is 32 random bytes as lowercase hex → 64 chars. */
const TOKEN_HEX_LENGTH = 64;

/** Pre-compiled regex for `looksLikeToken` — built once from `TOKEN_HEX_LENGTH`. */
const TOKEN_REGEX = new RegExp(`^[0-9a-f]{${TOKEN_HEX_LENGTH}}$`);

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
  return TOKEN_REGEX.test(value.trim());
}

/** Whether the popup's "Answer tools" disclosure should render pre-expanded.
 *  Defaults to `false` (collapsed) when never set. */
export async function getAnswerToolsExpanded(): Promise<boolean> {
  const stored = await browser.storage.local.get(ANSWER_TOOLS_EXPANDED_KEY);
  return stored[ANSWER_TOOLS_EXPANDED_KEY] === true;
}

/** Persist the Answer-tools disclosure's current expanded/collapsed state. */
export async function setAnswerToolsExpanded(expanded: boolean): Promise<void> {
  await browser.storage.local.set({ [ANSWER_TOOLS_EXPANDED_KEY]: expanded });
}
