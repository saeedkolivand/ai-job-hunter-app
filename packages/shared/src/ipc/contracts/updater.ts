export interface UpdaterContract {
  check(): Promise<void>;

  download(): Promise<void>;

  install(): Promise<void>;

  onStatus(handler: (status: unknown) => void): () => void;
}

export const UPDATER_CHANNELS = {
  check: 'updater:check',
  download: 'updater:download',
  install: 'updater:install',
  onStatus: 'updater:status',
} as const;
