/**
 * LinkedIn Session Store
 *
 * Persists LinkedIn authentication cookies for use by the HTTP-based
 * LinkedIn scraper client. Session capture is handled by PersistentBoardSession
 * (Electron partitions) — this class only reads and clears the saved data.
 */
import fs from 'node:fs/promises';
import path from 'node:path';

import { createLogger } from '@ajh/core';

const logger = createLogger('linkedin.session.store');

export interface LinkedInSessionData {
  cookies: Array<{ name: string; value: string; domain: string; path: string; expires?: number }>;
  li_at: string;
  JSESSIONID?: string;
  csrfToken?: string;
  lastUpdated: number;
}

export class LinkedInSessionStore {
  private readonly sessionPath: string;

  constructor(userDataDir: string) {
    this.sessionPath = path.join(userDataDir, 'linkedin-session-data.json');
  }

  /**
   * Load saved session data from disk.
   * Returns null if no session exists or if session is expired (> 24 hours).
   */
  async load(): Promise<LinkedInSessionData | null> {
    try {
      const data = await fs.readFile(this.sessionPath, 'utf-8');
      const session: LinkedInSessionData = JSON.parse(data);

      const SESSION_EXPIRY_MS = 24 * 60 * 60 * 1000;
      if (Date.now() - session.lastUpdated > SESSION_EXPIRY_MS) {
        logger.info('Session data expired');
        return null;
      }

      logger.info('Session data loaded successfully');
      return session;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
        logger.info('No session data found');
        return null;
      }
      logger.error({ error }, 'Failed to load session data');
      return null;
    }
  }

  /** Delete saved session data. */
  async clear(): Promise<void> {
    try {
      await fs.unlink(this.sessionPath);
      logger.info('Session data cleared');
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') {
        logger.error({ error }, 'Failed to clear session data');
      }
    }
  }

  /** Returns true if a non-expired session file exists. */
  async hasValidSession(): Promise<boolean> {
    return (await this.load()) !== null;
  }
}
