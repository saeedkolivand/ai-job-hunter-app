import { invoke } from '@tauri-apps/api/core';

export const linkedin = {
  connect: () => invoke('boards_login_with_browser', { boardId: 'linkedin' }),
  disconnect: () => invoke('boards_logout', { boardId: 'linkedin' }),
  getStatus: () => invoke('boards_get_status', { boardId: 'linkedin' }),
};
