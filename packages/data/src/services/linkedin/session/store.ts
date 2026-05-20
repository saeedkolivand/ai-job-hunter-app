/**
 * LinkedIn Session Store
 *
 * Manages persistence of LinkedIn authentication cookies and session data.
 * Extracts cookies from Playwright persistent contexts and saves them for HTTP client use.
 */
import path from 'node:path';
import fs from 'node:fs/promises';
import type { BrowserContext } from 'playwright';
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
   * Extract cookies from a Playwright persistent context and save them.
   * This should be called after successful authentication via Playwright.
   */
  async saveFromContext(context: BrowserContext): Promise<void> {
    try {
      const cookies = await context.cookies();
      const li_at = cookies.find((c) => c.name === 'li_at')?.value;
      const JSESSIONID = cookies.find((c) => c.name === 'JSESSIONID')?.value;

      if (!li_at) {
        logger.warn('No li_at cookie found in context');
        throw new Error('li_at cookie not found - authentication may have failed');
      }

      const sessionData: LinkedInSessionData = {
        cookies: cookies.map((c) => ({
          name: c.name,
          value: c.value,
          domain: c.domain,
          path: c.path,
          expires: c.expires,
        })),
        li_at,
        JSESSIONID,
        lastUpdated: Date.now(),
      };

      await fs.writeFile(this.sessionPath, JSON.stringify(sessionData, null, 2), 'utf-8');
      logger.info('Session data saved successfully');
    } catch (error) {
      logger.error({ error }, 'Failed to save session data');
      throw error;
    }
  }

  /**
   * Load saved session data from disk.
   * Returns null if no session exists or if session is expired (> 24 hours).
   */
  async load(): Promise<LinkedInSessionData | null> {
    try {
      const data = await fs.readFile(this.sessionPath, 'utf-8');
      const session: LinkedInSessionData = JSON.parse(data);

      // Check if session is expired (24 hours)
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

  /**
   * Extract cookies from a persistent context directory without launching a browser.
   * Reads cookies directly from the Playwright storage state file.
   */
  async extractFromPersistentDir(persistentDir: string): Promise<LinkedInSessionData | null> {
    try {
      const statePath = path.join(persistentDir, 'state.json');
      const stateData = await fs.readFile(statePath, 'utf-8');
      const state = JSON.parse(stateData);

      const cookies = state.cookies || [];
      const li_at = cookies.find((c: { name: string; value: string }) => c.name === 'li_at')?.value;
      const JSESSIONID = cookies.find(
        (c: { name: string; value: string }) => c.name === 'JSESSIONID'
      )?.value;

      if (!li_at) {
        logger.warn('No li_at cookie found in persistent context');
        return null;
      }

      const sessionData: LinkedInSessionData = {
        cookies: cookies.map(
          (c: {
            name: string;
            value: string;
            domain?: string;
            path?: string;
            secure?: boolean;
            httpOnly?: boolean;
            expires?: number;
            sameSite?: string;
          }) => ({
            name: c.name,
            value: c.value,
            domain: c.domain,
            path: c.path,
            expires: c.expires,
          })
        ),
        li_at,
        JSESSIONID,
        lastUpdated: Date.now(),
      };

      // Save to our own session file for easier access
      await fs.writeFile(this.sessionPath, JSON.stringify(sessionData, null, 2), 'utf-8');
      logger.info('Session extracted from persistent context');
      return sessionData;
    } catch (error) {
      logger.error({ error }, 'Failed to extract session from persistent context');
      return null;
    }
  }

  /**
   * Delete saved session data.
   */
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

  /**
   * Check if a valid session exists.
   */
  async hasValidSession(): Promise<boolean> {
    const session = await this.load();
    return session !== null;
  }
}
