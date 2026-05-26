export interface PrivacyContract {
  /** Sign out all connected accounts by wiping Chromium profiles. */
  signOutAll(): Promise<void>;

  /** Clear all saved job interaction history (applied, viewed, bookmarked). */
  clearInteractions(): Promise<void>;

  /** Factory reset: sign out all boards, clear all cached data. Frontend resets preferences separately. */
  resetApp(): Promise<void>;
}

export const PRIVACY_CHANNELS = {
  signOutAll: 'privacy:signOutAll',
  clearInteractions: 'privacy:clearInteractions',
  resetApp: 'privacy:resetApp',
} as const;
