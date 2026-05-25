import { invoke } from '@tauri-apps/api/core';

export const resume = {
  extractText: (req: unknown) => invoke('resume_extract_text', { req }),
};
