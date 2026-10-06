import { CORE_ENTRIES } from './search-index/entries-core';
import { DATA_ENTRIES } from './search-index/entries-data';
import { SYSTEM_ENTRIES } from './search-index/entries-system';

export type { SearchEntry } from './search-index/search-entry';

/** Every searchable entry, in nav order — each group lives in `./search-index/entries-*`. */
export const SEARCH_INDEX = [...CORE_ENTRIES, ...DATA_ENTRIES, ...SYSTEM_ENTRIES];
