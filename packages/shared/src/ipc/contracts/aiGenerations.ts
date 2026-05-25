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
