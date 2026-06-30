import { invoke } from '@tauri-apps/api/core';

import type { BoardCatalogEntry, CookieImportResult } from '@ajh/shared';

export const boards = {
  catalog: () => invoke<BoardCatalogEntry[]>('boards_catalog'),
  connect: ({ boardId }: { boardId: string }) => invoke('boards_login_with_browser', { boardId }),
  disconnect: ({ boardId }: { boardId: string }) => invoke('boards_logout', { boardId }),
  getStatus: ({ boardId }: { boardId: string }) => invoke('boards_get_status', { boardId }),
  importCookies: ({ boardId }: { boardId: string }) =>
    invoke<CookieImportResult>('boards_import_cookies', { boardId }),
};
