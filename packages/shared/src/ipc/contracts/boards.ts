export interface BoardsContract {
  /** Connect to a board by launching a browser for manual login. */
  connect(req: { boardId: string }): Promise<{ connected: boolean; accountEmail?: string }>;

  /** Disconnect a board (closes context only; does not delete profile). */
  disconnect(req: { boardId: string }): Promise<void>;

  /** Get current connection status for a board. */
  getStatus(req: {
    boardId: string;
  }): Promise<{ connected: boolean; accountEmail?: string; lastConnected?: number }>;
}

export const BOARDS_CHANNELS = {
  connect: 'boards:connect',
  disconnect: 'boards:disconnect',
  getStatus: 'boards:getStatus',
} as const;
