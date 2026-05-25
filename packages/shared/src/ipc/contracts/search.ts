import type { HybridSearchRequest } from '../../schemas/index.js';
import type { SearchHit } from '../../types/index.js';

export interface SearchContract {
  hybrid(req: HybridSearchRequest): Promise<SearchHit[]>;
}

export const SEARCH_CHANNELS = {
  hybrid: 'search:hybrid',
} as const;
