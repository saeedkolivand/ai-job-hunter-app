import type { MatchResumeRequest } from '../../schemas/index.js';
import type { MatchScore } from '../../types/index.js';

export interface MatchContract {
  resume(req: MatchResumeRequest): Promise<MatchScore>;
}

export const MATCH_CHANNELS = {
  resume: 'match:resume',
} as const;
