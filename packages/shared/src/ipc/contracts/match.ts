import type { MatchResumeBatchRequest, MatchResumeRequest } from '../../schemas/index.js';
import type { MatchScore } from '../../types/index.js';

export interface MatchContract {
  resume(req: MatchResumeRequest): Promise<MatchScore>;
  resumeBatch(req: MatchResumeBatchRequest): Promise<MatchScore[]>;
}

export const MATCH_CHANNELS = {
  resume: 'match:resume',
  resumeBatch: 'match:resume_batch',
} as const;
