import { invoke } from '@tauri-apps/api/core';

import type { CookieImportResult } from '@ajh/shared';

export const boards = {
  connect: ({ boardId }: { boardId: string }) => invoke('boards_connect', { boardId }),
  disconnect: ({ boardId }: { boardId: string }) => invoke('boards_disconnect', { boardId }),
  getStatus: ({ boardId }: { boardId: string }) => invoke('boards_get_status', { boardId }),
  importCookies: ({ boardId }: { boardId: string }) =>
    invoke<CookieImportResult>('boards_import_cookies', { boardId }),
};
