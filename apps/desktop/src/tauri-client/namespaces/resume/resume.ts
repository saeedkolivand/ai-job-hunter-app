import { invoke } from '@tauri-apps/api/core';

import type { ResumeExtractTextRequest, ResumeValidateContentRequest } from '@ajh/shared/schemas';

export const resume = {
  extractText: (req: ResumeExtractTextRequest) => invoke('resume_extract_text', { req }),
  validateContent: (req: ResumeValidateContentRequest) =>
    invoke('resume_validate_content', { req }),
};
