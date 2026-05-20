/**
 * Scraper interface — every job board implements this.
 *
 * Each scraper MUST normalize results into the shared `JobPosting` shape
 * before returning. Source-specific quirks are isolated per implementation.
 */
import type { JobPosting } from '@ajh/shared';

import type { BrowserController } from './browser.js';

/** Credentials are resolved per-board, on demand, inside the main process. */
export interface CredentialsAccessor {
  /** Returns plaintext credentials if the user has saved any for this board. */
  get(boardId: string): Promise<{ username: string; password: string } | null>;
  /** Path Playwright should use for this board's persistent state. */
  storageStatePath(boardId: string): string;
}

export interface ScrapeContext {
  signal: AbortSignal;
  onProgress?: (p: number) => void;
  onItem?: (item: JobPosting) => void | Promise<void>;
  /** Lazy access to Playwright. Always provided by the bootstrap runner. */
  browser?: BrowserController;
  /** Credential lookup. Always provided by the bootstrap runner. */
  credentials?: CredentialsAccessor;
}

export interface BoardSearchInput {
  query: string;
  location?: string;
  pages: number;
  dateFilter?: string;
  jobType?: string; // 'F' (Full-time), 'P' (Part-time), 'C' (Contract), 'T' (Temporary), 'I' (Internship), 'V' (Volunteer)
  workType?: string; // '1' (On-site), '2' (Remote), '3' (Hybrid)
  experienceLevel?: string; // '1' (Internship), '2' (Entry), '3' (Associate), '4' (Mid-Senior), '5' (Director), '6' (Executive)
  easyApply?: boolean;
  activelyHiring?: boolean;
  verified?: boolean;
  sortBy?: string; // 'DD' (Date Descending), 'R' (Relevance)
  locale?: string; // board-specific locale/country code, e.g. 'de', 'uk', 'fr'
}

export interface Scraper {
  readonly id: string;
  readonly displayName: string;
  readonly mode: 'http' | 'browser';
  search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]>;
  fromUrl?(url: string, ctx: ScrapeContext): Promise<JobPosting | null>;
}

export abstract class BaseScraper implements Scraper {
  abstract readonly id: string;
  abstract readonly displayName: string;
  abstract readonly mode: 'http' | 'browser';
  abstract search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]>;

  protected makeId(externalId: string): string {
    return `${this.id}:${externalId}`;
  }
}
