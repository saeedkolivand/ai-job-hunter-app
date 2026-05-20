/**
 * LinkedIn Authentication Manager
 *
 * Manages LinkedIn authentication using Playwright ONLY for login.
 * After successful login, extracts and saves session cookies for HTTP client use.
 */
import path from 'node:path';

import { type BrowserContext, chromium } from 'playwright';

import { createLogger } from '@ajh/core';

import { type LinkedInSessionData, LinkedInSessionStore } from '../session/store.js';

const logger = createLogger('linkedin.auth.manager');

export interface AuthManagerOptions {
  userDataDir: string;
  headless?: boolean;
}

export class LinkedInAuthManager {
  private readonly sessionStore: LinkedInSessionStore;
  private readonly userDataDir: string;
  private readonly headless: boolean;
  private context?: BrowserContext;

  constructor(options: AuthManagerOptions) {
    this.userDataDir = path.join(options.userDataDir, 'linkedin-auth-context');
    this.headless = options.headless ?? false; // Default to visible for first-time login
    this.sessionStore = new LinkedInSessionStore(options.userDataDir);
  }

  /**
   * Authenticate with LinkedIn using Playwright.
   * Opens a browser window for manual login, then extracts and saves session cookies.
   * This is the ONLY place Playwright should be used for LinkedIn.
   */
  async authenticate(): Promise<LinkedInSessionData> {
    logger.info('Starting LinkedIn authentication flow');

    // Close existing context if any
    await this.disconnect();

    // Launch persistent context for authentication
    this.context = await chromium.launchPersistentContext(this.userDataDir, {
      headless: this.headless,
      viewport: { width: 1366, height: 900 },
      userAgent:
        'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36',
      locale: 'en-US',
      timezoneId: 'Europe/Berlin',
      args: ['--disable-blink-features=AutomationControlled'],
    });

    // Get or create a page
    const pages = this.context.pages();
    const page = pages.length > 0 ? pages[0] : await this.context.newPage();
    if (!page) {
      throw new Error('Failed to create or get page');
    }

    // Navigate to LinkedIn login
    await page.goto('https://www.linkedin.com/login', {
      waitUntil: 'domcontentloaded',
      timeout: 60_000,
    });

    // Wait for user to complete login (up to 5 minutes)
    logger.info('Waiting for user to complete login...');
    await page.waitForFunction(
      () => {
        const url = window.location.href;
        return (
          (!url.includes('/login') && !url.includes('/uas/login')) ||
          url.includes('/feed') ||
          url.includes('/checkpoint') ||
          url.includes('/challenge')
        );
      },
      { timeout: 300_000 }
    );

    // Validate login by checking if we're on an authenticated page
    const currentUrl = page.url();
    if (currentUrl.includes('/login') || currentUrl.includes('/uas/login')) {
      throw new Error('Authentication failed - still on login page');
    }

    logger.info('Login successful, extracting session cookies');

    // Extract and save session cookies
    await this.sessionStore.saveFromContext(this.context);

    // Load the saved session data
    const sessionData = await this.sessionStore.load();
    if (!sessionData) {
      throw new Error('Failed to load saved session data');
    }

    // Close the browser - we only needed it for authentication
    await this.context.close();
    this.context = undefined;

    logger.info('Authentication complete, session saved');
    return sessionData;
  }

  /**
   * Check if a valid session exists.
   */
  async hasValidSession(): Promise<boolean> {
    return this.sessionStore.hasValidSession();
  }

  /**
   * Load existing session data.
   */
  async loadSession(): Promise<LinkedInSessionData | null> {
    return this.sessionStore.load();
  }

  /**
   * Extract session from an existing persistent context directory.
   * Used for migrating existing LinkedIn sessions to the new format.
   */
  async extractFromPersistentContext(persistentDir: string): Promise<LinkedInSessionData | null> {
    return this.sessionStore.extractFromPersistentDir(persistentDir);
  }

  /**
   * Clear saved session data.
   */
  async clearSession(): Promise<void> {
    await this.sessionStore.clear();
  }

  /**
   * Disconnect and clean up the authentication context.
   */
  async disconnect(): Promise<void> {
    if (this.context) {
      logger.info('Closing authentication context');
      await this.context.close().catch(() => {});
      this.context = undefined;
    }
  }

  /**
   * Clean shutdown.
   */
  async close(): Promise<void> {
    await this.disconnect();
  }
}
