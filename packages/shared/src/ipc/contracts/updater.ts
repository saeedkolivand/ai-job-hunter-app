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

/** Result of {@link UpdaterContract.check}. Mirrors the shell's `updater_check`
 *  JSON: an available update with its version, no update, or an error string.
 *  `downloaded`/`downloading` are only ever `true` together with `available`
 *  — the backend refuses to discard a download already finished or in
 *  flight, and reports that state back instead of re-fetching, so a
 *  returning caller can re-attach rather than restart. Detailed progress
 *  still arrives via the `updater:status` event stream. */
export type UpdateCheckResult =
  | { available: true; version: string; downloaded?: boolean; downloading?: boolean }
  | {
      available: false;
      /** Present only on a **packaged** install — Microsoft Store (MSIX) or
       *  Snap: that flavour's store/sandbox delivers updates for it, so the
       *  shell answers without ever contacting GitHub — no check, no
       *  background poll, and `download`/`install` refuse. Absent on every
       *  other install, where `available: false` keeps its plain "you are
       *  up to date" meaning. */
      managedBy?: 'msstore' | 'snap';
    }
  | { error: string };

export interface UpdaterContract {
  /** Trigger a check. Resolves with the outcome (also emitted on `onStatus`). */
  check(): Promise<UpdateCheckResult>;

  /** Download the update {@link check} found. Resolves either way — the shell
   *  reports failure on the `updater:status` stream, not by rejecting. On a
   *  packaged install the shell refuses instead (the store/sandbox owns
   *  updating); that refusal is defence in depth for a non-renderer caller
   *  such as the agent CLI, since this signature discards it and the UI never
   *  offers the action once it has seen a `managedBy` value. */
  download(): Promise<void>;

  /** Install the downloaded update and relaunch. Same packaged-install
   *  caveat as {@link download}. */
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
} as const;
