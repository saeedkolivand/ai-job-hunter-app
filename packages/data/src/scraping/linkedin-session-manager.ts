/**
 * @deprecated This file is deprecated. Use BoardSessionManager instead.
 * Kept for backward compatibility - will be removed in future version.
 */
import { BoardSessionManager } from './board-session-manager.js';

export interface LinkedInSessionStatus {
  connected: boolean;
  lastConnected?: number;
  accountEmail?: string;
  sessionPath?: string;
}

/**
 * @deprecated Use BoardSessionManager instead
 */
export class LinkedInSessionManager {
  private boardManager: BoardSessionManager;

  constructor(userDataDir: string) {
    // Redirect to BoardSessionManager with LinkedIn config
    this.boardManager = new BoardSessionManager(userDataDir, {
      id: 'linkedin',
      displayName: 'LinkedIn',
      loginUrl: 'https://www.linkedin.com/login',
      validateUrl: 'https://www.linkedin.com/feed/',
      isAuthenticatedUrl: (u) =>
        !u.includes('/login') && !u.includes('/uas/login') && !u.includes('/checkpoint/lg'),
      legacyDir: 'linkedin-session',
    });
  }

  async connect(): Promise<LinkedInSessionStatus> {
    const status = await this.boardManager.connect();
    return status;
  }

  async getStatus(): Promise<LinkedInSessionStatus> {
    const status = await this.boardManager.getStatus();
    return status;
  }

  async disconnect(): Promise<void> {
    await this.boardManager.disconnect();
  }

  async close(): Promise<void> {
    await this.boardManager.close();
  }
}
