export interface AiGenerationRecord {
  id: string;
  createdAt: number;
  candidateName: string;
  jobTitle: string;
  companyName: string;
  resumeLanguage: string;
  jobAdLanguage: string;
  targetLanguage: string;
  mismatch: boolean;
  topRequirements: string[];
  mode: string;
  resumeText: string;
  coverLetterText: string;
  jobAd: string;
  /** The job this generation targets — links the record to an autopilot found job. */
  jobUrl: string;
  /** The board the job came from (e.g. "linkedin"). */
  board: string;
}

export interface AiGenerationSaveRequest {
  candidateName: string;
  jobTitle: string;
  companyName: string;
  resumeLanguage: string;
  jobAdLanguage: string;
  targetLanguage: string;
  mismatch: boolean;
  topRequirements: string[];
  mode: string;
  resumeText: string;
  coverLetterText: string;
  jobAd: string;
  /** The job this generation targets (marks the autopilot found job "applied"). */
  jobUrl?: string;
  /** The board the job came from. */
  board?: string;
}

export interface AiGenerationsContract {
  list(): Promise<AiGenerationRecord[]>;
  save(req: AiGenerationSaveRequest): Promise<{ id: string; success: boolean }>;
  remove(id: string): Promise<void>;
}

export const AI_GENERATIONS_CHANNELS = {
  list: 'aiGenerations:list',
  save: 'aiGenerations:save',
  remove: 'aiGenerations:remove',
} as const;
