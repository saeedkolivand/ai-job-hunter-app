import { invoke } from '@tauri-apps/api/core';

import type { ResumeExtractTextRequest } from '@ajh/shared/schemas';

export const resume = {
  extractText: (req: ResumeExtractTextRequest) => invoke('resume_extract_text', { req }),
};
