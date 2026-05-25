import { invoke } from '@tauri-apps/api/core';

export const credentials = {
  available: () => invoke('credentials_available'),
  list: () => invoke('credentials_list'),
  set: (req: unknown) => invoke('credentials_set', { req }),
  remove: ({ boardId }: { boardId: string }) => invoke('credentials_remove', { boardId }),
};
