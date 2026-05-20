/**
 * Board session registry.
 *
 * Creates one PersistentBoardSession per supported board and exports them
 * as a Map for use in bootstrap.ts and the IPC router.
 *
 * On startup, exportCookies() is called for any board that already has a
 * valid session so the scrapers have fresh state.json files without
 * requiring the user to log in again.
 */
import { createLogger } from '@ajh/core';

import { BOARD_CONFIGS } from './configs.js';
import { PersistentBoardSession } from './PersistentBoardSession.js';

const logger = createLogger('board-sessions');

export { BOARD_CONFIGS } from './configs.js';
export { PersistentBoardSession } from './PersistentBoardSession.js';
export type { BoardConfig, BoardSessionStatus } from './types.js';

export type BoardSessionMap = Map<string, PersistentBoardSession>;

/**
 * Create sessions for all configured boards and refresh state.json files
 * for any board that has a still-valid persistent session.
 */
export async function createBoardSessions(userDataDir: string): Promise<BoardSessionMap> {
  const map: BoardSessionMap = new Map();

  for (const boardId of Object.keys(BOARD_CONFIGS)) {
    const sess = new PersistentBoardSession(userDataDir, boardId);
    map.set(boardId, sess);

    // Refresh state.json on startup for boards with existing valid sessions.
    // This ensures scrapers work immediately without requiring a re-login.
    const status = await sess.getStatus().catch(() => ({ connected: false }));
    if (status.connected) {
      await sess
        .exportCookies()
        .catch((err) => logger.warn({ boardId, err }, 'startup exportCookies failed'));
      logger.info({ boardId }, 'existing session found — state.json refreshed');
    }
  }

  return map;
}
