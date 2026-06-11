import { invoke } from '@tauri-apps/api/core';

import type { CookieImportResult } from '@ajh/shared';

export const linkedin = {
  connect: () => invoke('boards_login_with_browser', { boardId: 'linkedin' }),
  disconnect: () => invoke('boards_logout', { boardId: 'linkedin' }),
  getStatus: () => invoke('boards_get_status', { boardId: 'linkedin' }),
  importProfileFromUrl: (url: string) => invoke('profile_import_from_url', { url }),
  importCookies: () => invoke<CookieImportResult>('boards_import_cookies', { boardId: 'linkedin' }),
};
