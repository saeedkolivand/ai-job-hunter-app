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

export interface PrivacyContract {
  /** Sign out all connected accounts by wiping Chromium profiles. */
  signOutAll(): Promise<void>;

  /** Clear all saved job interaction history (applied, viewed, bookmarked). */
  clearInteractions(): Promise<void>;

  /** Factory reset: sign out all boards, clear all cached data. Frontend resets preferences separately. */
  resetApp(): Promise<PrivacyResetResult>;
}

export const PRIVACY_CHANNELS = {
  signOutAll: 'privacy:signOutAll',
  clearInteractions: 'privacy:clearInteractions',
  resetApp: 'privacy:resetApp',
} as const;
