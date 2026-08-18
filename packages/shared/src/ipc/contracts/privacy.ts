/**
 * Outcome of a factory reset. The Rust `privacy_reset_app` command wipes every
 * persistent store and then removes the on-disk Chromium board-login profiles.
 * Store-wipe always succeeds; the profile removal is best-effort (a browser may
 * still hold a file lock, common on Windows), so a partial reset is reported
 * honestly instead of over-claiming a clean wipe.
 *
 * - `{ success: true }` — full reset (stores wiped + browser state removed).
 * - `{ success: false, error, browserStateRetained: true }` — partial reset:
 *   stores were wiped but board-login sessions remain on disk.
 */
export interface PrivacyResetResult {
  success: boolean;
  error?: string;
  browserStateRetained?: boolean;
}

/**
 * Crash-reporting consent. Rust-owned rather than a renderer preference: the
 * Sentry client is constructed before `tauri::Builder` runs (the minidump
 * supervisor forks at startup), so there is no WebView — and therefore no
 * `localStorage` — to read at the moment the decision is needed.
 *
 * `enabled` is the user's choice and defaults to **true**. `consentShown`
 * records whether the setup wizard has actually put that choice in front of
 * them; nothing is transmitted until it has, so a default nobody saw never
 * silently reports. Transmission requires BOTH flags (`Settings::transmits`).
 *
 * These two are the whole surface — see ADR-0020. There is no analytics
 * toggle and no retention setting because there is no behavioural analytics to
 * switch off, and retention belongs to the processor, not to the app.
 */
export interface CrashReportingSettings {
  enabled: boolean;
  consentShown: boolean;
}

export interface PrivacyContract {
  /** Sign out all connected accounts by wiping Chromium profiles. */
  signOutAll(): Promise<void>;

  /** Clear all saved job interaction history (applied, viewed, bookmarked). */
  clearInteractions(): Promise<void>;

  /** Factory reset: sign out all boards, clear all cached data. Frontend resets preferences separately. */
  resetApp(): Promise<PrivacyResetResult>;

  /** Current crash-reporting consent state. */
  getCrashReporting(): Promise<CrashReportingSettings>;

  /**
   * Persist crash-reporting consent.
   *
   * Turning it OFF unbinds the client immediately, so no further error or
   * breadcrumb events are captured in this session. The native-crash supervisor
   * is a separate process forked at launch and cannot be recalled mid-session,
   * so a hard crash before the next restart could still be reported — the UI
   * says so rather than implying a clean instant stop.
   */
  setCrashReporting(settings: CrashReportingSettings): Promise<CrashReportingSettings>;
}

export const PRIVACY_CHANNELS = {
  signOutAll: 'privacy:signOutAll',
  clearInteractions: 'privacy:clearInteractions',
  resetApp: 'privacy:resetApp',
  getCrashReporting: 'privacy:getCrashReporting',
  setCrashReporting: 'privacy:setCrashReporting',
} as const;
