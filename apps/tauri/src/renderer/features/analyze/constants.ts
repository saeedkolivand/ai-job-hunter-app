export const ACCEPTED_EXTS = ['pdf', 'docx', 'txt', 'md', 'markdown'] as const;
export const MAX_BYTES = 25 * 1024 * 1024;

export type Stage = 'idle' | 'running' | 'done';
