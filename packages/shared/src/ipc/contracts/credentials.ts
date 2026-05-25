import type { CredentialMetadata } from '../../types/index.js';

export interface CredentialsContract {
  /** Whether the OS supports encrypted secret storage. */
  available(): Promise<boolean>;

  /** Returns metadata only — NEVER passwords. */
  list(): Promise<CredentialMetadata[]>;

  set(req: { boardId: string; username: string; password: string }): Promise<void>;

  remove(req: { boardId: string }): Promise<void>;
}

export const CREDENTIALS_CHANNELS = {
  available: 'credentials:available',
  list: 'credentials:list',
  set: 'credentials:set',
  remove: 'credentials:remove',
} as const;
