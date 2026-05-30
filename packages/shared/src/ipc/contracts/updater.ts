/** One release entry surfaced in the in-app changelog. */
export interface ChangelogRelease {
  /** Version without a leading `v` (e.g. `"0.28.0"`). */
  version: string;
  /** Release title, if GitHub has one. */
  name: string | null;
  /** Release notes body (Markdown), if any. */
  body: string | null;
  /** ISO 8601 publish timestamp, if any. */
  publishedAt: string | null;
  /** GitHub release page URL. */
  url: string;
  prerelease: boolean;
}

/** Result of {@link UpdaterContract.changelog}. Never rejects — errors surface here. */
export interface ChangelogResult {
  releases?: ChangelogRelease[];
  error?: string;
}

export interface UpdaterContract {
  check(): Promise<void>;

  download(): Promise<void>;

  install(): Promise<void>;

  /** Recent release history (newest first) for the in-app changelog. */
  changelog(): Promise<ChangelogResult>;

  onStatus(handler: (status: unknown) => void): () => void;
}

export const UPDATER_CHANNELS = {
  check: 'updater:check',
  download: 'updater:download',
  install: 'updater:install',
  changelog: 'updater:changelog',
  onStatus: 'updater:status',
} as const;
