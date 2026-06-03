export interface CredentialsContract {
  /** Whether the OS supports encrypted secret storage. Board logins use browser
   *  sessions (`boards.*`), so there is no password CRUD here; this only gates
   *  the encryption-availability warning. */
  available(): Promise<boolean>;
}

export const CREDENTIALS_CHANNELS = {
  available: 'credentials:available',
} as const;
