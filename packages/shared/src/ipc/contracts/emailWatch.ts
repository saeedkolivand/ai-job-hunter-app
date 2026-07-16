/**
 * Email-confirmation watching (Task #23, auto-track Layer C) — IMAP
 * connect/status/enable control surface.
 *
 * **PR A scope only**: connect/status/enable/disconnect + a manual
 * connectivity re-check. The backend validates the address/app-password by a
 * real IMAP `LOGIN` + `SELECT INBOX` before persisting anything; nothing is
 * fetched or parsed from the mailbox yet — the poller/parser/matcher land in
 * PR B. `appPassword` is write-only: sent once to `connect`, stored in the OS
 * keychain, and never returned or logged.
 */

/** Current connection status. `connected` means an account has been
 *  configured (a successful `connect`), not that a live socket is open —
 *  there is no persistent IMAP connection; each check connects fresh. */
export interface EmailWatchStatus {
  connected: boolean;
  address?: string;
  /** The (future) poller opt-in — default OFF, independent of `connected`. */
  enabled: boolean;
  lastCheckAt?: number;
  lastMatchAt?: number;
}

export interface EmailWatchConnectRequest {
  address: string;
  appPassword: string;
}

export interface EmailWatchContract {
  status(): Promise<EmailWatchStatus>;
  /** Validates by a real IMAP LOGIN + SELECT INBOX before persisting. */
  connect(req: EmailWatchConnectRequest): Promise<EmailWatchStatus>;
  /** Removes the keychain app password and clears the account row. */
  disconnect(): Promise<EmailWatchStatus>;
  setEnabled(enabled: boolean): Promise<EmailWatchStatus>;
  /** Re-validates the existing connection (LOGIN + SELECT INBOX). Fetches no
   *  mail — see the module doc. */
  checkNow(): Promise<EmailWatchStatus>;
}

export const EMAIL_WATCH_CHANNELS = {
  status: 'emailWatch:status',
  connect: 'emailWatch:connect',
  disconnect: 'emailWatch:disconnect',
  setEnabled: 'emailWatch:setEnabled',
  checkNow: 'emailWatch:checkNow',
} as const;
