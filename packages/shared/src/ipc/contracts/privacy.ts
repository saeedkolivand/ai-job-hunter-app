export interface PrivacyContract {
  /** Sign out all connected accounts by wiping Chromium profiles. */
  signOutAll(): Promise<void>;

  /** Clear all saved job interaction history (applied, viewed, bookmarked). */
  clearInteractions(): Promise<void>;
}

export const PRIVACY_CHANNELS = {
  signOutAll: 'privacy:signOutAll',
  clearInteractions: 'privacy:clearInteractions',
} as const;
