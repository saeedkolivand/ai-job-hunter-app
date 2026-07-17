/**
 * Email-confirmation watching (Task #23, auto-track Layer C) — IMAP
 * connect/status/enable control surface, plus the poller it gates.
 *
 * The backend validates the address/app-password by a real IMAP `LOGIN` +
 * `SELECT INBOX` before persisting anything (`connect`). Once `enabled`, a
 * backend-owned background poller periodically fetches new INBOX headers,
 * fingerprints them as plausible application-confirmation emails, and
 * fuzzy-matches company/title against the user's saved applications — a
 * match surfaces as a Notification Center card (never an auto-write; the
 * user still marks applied themselves). `checkNow` runs that SAME pass
 * on-demand, gated by a short server-side min-interval guard (rejects if a
 * check ran too recently) so it can't be used to spam Gmail logins.
 * `appPassword` is write-only: sent once to `connect`, stored in the OS
 * keychain, and never returned or logged.
 */

/** Current connection status. `connected` means an account has been
 *  configured (a successful `connect`), not that a live socket is open —
 *  there is no persistent IMAP connection; each check connects fresh. */
export interface EmailWatchStatus {
  connected: boolean;
  address?: string;
  /** The poller opt-in — default OFF, independent of `connected`. */
  enabled: boolean;
  lastCheckAt?: number;
  /** Timestamp of the most recent email→application match, if any. */
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
  /** Runs a real fetch+parse+match+notify pass now (the same pass the
   *  background poller runs). Rejects if a check already ran too recently. */
  checkNow(): Promise<EmailWatchStatus>;
}

export const EMAIL_WATCH_CHANNELS = {
  status: 'emailWatch:status',
  connect: 'emailWatch:connect',
  disconnect: 'emailWatch:disconnect',
  setEnabled: 'emailWatch:setEnabled',
  checkNow: 'emailWatch:checkNow',
} as const;
