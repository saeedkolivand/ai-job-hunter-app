import { invoke } from '@tauri-apps/api/core';

import type { AiGenerationSaveRequest, AiGenerationUpdateRequest } from '@ajh/shared';

export const aiGenerations = {
  list: () => invoke('ai_generations_list'),
  save: (req: AiGenerationSaveRequest) => invoke('ai_generations_save', { req }),
  update: (req: AiGenerationUpdateRequest) => invoke('ai_generations_update', { req }),
  remove: (id: string) => invoke('ai_generations_remove', { id }),
};
