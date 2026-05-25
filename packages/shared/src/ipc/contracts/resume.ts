export interface ResumeContract {
  /** Extract plain text from an uploaded resume/job-ad file (pdf, docx, txt, md). */
  extractText(req: { name: string; bytes: Uint8Array }): Promise<{ text: string }>;
}

export const RESUME_CHANNELS = {
  extractText: 'resume:extractText',
} as const;
