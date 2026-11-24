/**
 * The settings that follow the user to another device through the optional
 * self-hosted sync (#1433). Backend-owned: the shell keeps an allowlist of keys
 * (performance, local model limits, launch-at-login, close-to-tray, consent
 * toggles and every secret are NOT on it) and merges two devices per key, last
 * writer wins. Values are opaque JSON; the renderer validates them on read.
 */

/** Every stored key with its current value. */
export interface PortablePrefsSnapshot {
  values: Record<string, unknown>;
  /** The one-time copy from the renderer's own storage already ran. */
  migrated: boolean;
}

export interface PortablePrefsContract {
  get(): Promise<PortablePrefsSnapshot>;
  /**
   * Record the user's changes. Rejects the whole call when a key is outside the
   * allowlist or a value is null or too large; a value equal to the stored one
   * is not a change.
   */
  set(values: Record<string, unknown>): Promise<{ success: true }>;
  /**
   * One-time import of the renderer's existing values. Fills only keys the
   * backend does not hold (a value received by sync is never overwritten) and
   * does nothing once it has run. Returns the resulting snapshot.
   */
  migrate(values: Record<string, unknown>): Promise<PortablePrefsSnapshot>;
}

export const PORTABLE_PREFS_CHANNELS = {
  get: 'portable_prefs_get',
  set: 'portable_prefs_set',
  migrate: 'portable_prefs_migrate',
} as const;
