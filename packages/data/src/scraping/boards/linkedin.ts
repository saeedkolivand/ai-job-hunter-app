/**
 * LinkedIn — uses HTTP API for scraping, Playwright ONLY for authentication.
 *
 * Architecture:
 *   - Authentication: Playwright via BoardSessionManager (user logs in once)
 *   - Scraping: HTTP requests via LinkedInJobsApiClient (no browser)
 *   - Session: Cookies extracted from Playwright and reused for HTTP requests
 *
 * APIs used:
 *   - Guest API (no auth): /jobs-guest/jobs/api/seeMoreJobPostings/search
 *   - Authenticated API: Same endpoint with session cookies for gated jobs
 *   - Job details: /jobs-guest/jobs/api/jobPosting/{id}
 *
 * Benefits:
 *   - Dramatically faster (no browser overhead)
 *   - Lower CPU/RAM usage
 *   - API-based pagination (no scrolling)
 *   - Rate limiting with exponential backoff
 */
import path from 'node:path';
import { existsSync } from 'node:fs';
import type { JobPosting } from '@ajh/shared';
import { BaseScraper, type BoardSearchInput, type ScrapeContext } from '../base.js';
import {
  LinkedInJobsApiClient,
  LinkedInHttpClient,
  type LinkedInSessionData,
} from '../../services/linkedin/index.js';

const _PAGE_SIZE = 25;

export class LinkedInScraper extends BaseScraper {
  readonly id = 'linkedin';
  readonly displayName = 'LinkedIn';
  readonly mode = 'http' as const;

  async search(input: BoardSearchInput, ctx: ScrapeContext): Promise<JobPosting[]> {
    // Check for authenticated session - use both new and legacy paths
    const sessionDir = this.resolveSessionDir(ctx);
    const hasSession = !!sessionDir && existsSync(sessionDir);

    // Load session data if available
    let sessionData: LinkedInSessionData | null = null;
    if (hasSession && sessionDir) {
      try {
        sessionData = await this.loadSessionData(sessionDir);
      } catch (error) {
        // Session file doesn't exist or is invalid, fall back to guest API
        console.warn('Failed to load session data, using guest API:', error);
      }
    }

    // Create HTTP client with session
    const httpClient = new LinkedInHttpClient({
      sessionData: sessionData ?? undefined,
      signal: ctx.signal,
    });
    const apiClient = new LinkedInJobsApiClient(httpClient);

    // Use authenticated API if session exists, otherwise guest API
    const maxPages = Math.min(Math.max(input.pages, 1), 10);

    return apiClient.searchPaginated(
      {
        keywords: input.query,
        location: input.location,
        dateFilter: input.dateFilter,
        jobType: input.jobType,
        workType: input.workType,
        experienceLevel: input.experienceLevel,
        easyApply: input.easyApply,
        activelyHiring: input.activelyHiring,
        verified: input.verified,
        sortBy: input.sortBy,
      },
      maxPages,
      ctx.signal,
      ctx.onProgress,
      ctx.onItem
    );
  }

  /**
   * Resolve the persistent LinkedIn session directory.
   * Tries both new (board-sessions/linkedin) and legacy (linkedin-session) locations.
   * Only returns a directory if the state.json file actually exists.
   */
  private resolveSessionDir(ctx: ScrapeContext): string | null {
    const statePath = ctx.credentials?.storageStatePath(this.id);
    if (!statePath) return null;
    const userData = path.dirname(path.dirname(statePath));

    // Try new location first
    const newDir = path.join(userData, 'board-sessions', this.id);
    const newStateFile = path.join(newDir, 'state.json');
    if (existsSync(newStateFile)) {
      console.warn('Using new session directory:', newDir);
      return newDir;
    }

    // Fall back to legacy location
    const legacyDir = path.join(userData, 'linkedin-session');
    const legacyStateFile = path.join(legacyDir, 'state.json');
    if (existsSync(legacyStateFile)) {
      console.warn('Using legacy session directory:', legacyDir);
      return legacyDir;
    }

    console.warn('No valid session found (state.json not found in either location)');
    return null;
  }

  /**
   * Load session data from persistent context directory.
   * Extracts cookies from Playwright's state.json file.
   */
  private async loadSessionData(sessionDir: string): Promise<LinkedInSessionData | null> {
    try {
      const statePath = path.join(sessionDir, 'state.json');
      const fs = await import('node:fs/promises');
      const stateData = await fs.readFile(statePath, 'utf-8');
      const state = JSON.parse(stateData);

      const cookies = state.cookies || [];
      const li_at = cookies.find((c: { name: string }) => c.name === 'li_at')?.value;
      const JSESSIONID = cookies.find((c: { name: string }) => c.name === 'JSESSIONID')?.value;

      if (!li_at) {
        console.warn('No li_at cookie found in session');
        return null;
      }

      console.warn('Successfully loaded LinkedIn session data');
      return {
        cookies: cookies.map(
          (c: {
            name: string;
            value: string;
            domain?: string;
            path?: string;
            secure?: boolean;
            httpOnly?: boolean;
            expires?: number;
            sameSite?: string;
          }) => ({
            name: c.name,
            value: c.value,
            domain: c.domain,
            path: c.path,
            expires: c.expires,
          })
        ),
        li_at,
        JSESSIONID,
        lastUpdated: Date.now(),
      };
    } catch (error) {
      console.error('Failed to load session data:', error);
      throw error; // Re-throw to let caller handle the error
    }
  }
}
