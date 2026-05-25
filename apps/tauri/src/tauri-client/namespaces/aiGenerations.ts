import { invoke } from '@tauri-apps/api/core';

export const aiGenerations = {
  list: () => invoke('ai_generations_list'),
  save: (req: unknown) => invoke('ai_generations_save', { req }),
  remove: (id: string) => invoke('ai_generations_remove', { id }),
};
