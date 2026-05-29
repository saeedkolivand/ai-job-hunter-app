import { invoke } from '@tauri-apps/api/core';

export const boards = {
  connect: ({ boardId }: { boardId: string }) => invoke('boards_connect', { boardId }),
  disconnect: ({ boardId }: { boardId: string }) => invoke('boards_disconnect', { boardId }),
  getStatus: ({ boardId }: { boardId: string }) => invoke('boards_get_status', { boardId }),
};
