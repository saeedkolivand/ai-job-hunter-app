/**
 * LinkedIn Jobs API Client (Guest API)
 * Fetches job postings from LinkedIn's guest API without authentication.
 * Limited to basic search parameters and a small number of results.
 */

import { load } from 'cheerio';

import { createLogger } from '@ajh/core';
import type { JobPosting } from '@ajh/shared';

import type { LinkedInHttpClient } from '../client/http-client.js';
import type { LinkedInSessionData } from '../session/store.js';

const logger = createLogger('linkedin.jobs.api');

// LinkedIn API pagination
const PAGE_SIZE = 25;

// Helper function to strip HTML tags
function stripHtml(html: string): string {
  return html.replace(/<[^>]*>/g, '').trim();
}

export interface JobsSearchParams {
  keywords: string;
  location?: string;
  start?: number;
  dateFilter?: string; // '24h', '8h', 'week', 'month'
  jobType?: string; // 'F' (Full-time), 'P' (Part-time), 'C' (Contract), 'T' (Temporary), 'I' (Internship), 'V' (Volunteer)
  workType?: string; // '1' (On-site), '2' (Remote), '3' (Hybrid)
  experienceLevel?: string; // '1' (Internship), '2' (Entry), '3' (Associate), '4' (Mid-Senior), '5' (Director), '6' (Executive)
  easyApply?: boolean;
  activelyHiring?: boolean;
  verified?: boolean;
  sortBy?: string; // 'DD' (Date Descending), 'R' (Relevance)
}

export interface LinkedInJobCard {
  urn: string;
  title: string;
  company: string;
  location: string;
  link: string;
  postedAt?: string;
}

export class LinkedInJobsApiClient {
  private client: LinkedInHttpClient;

  constructor(client: LinkedInHttpClient) {
    this.client = client;
  }

  /**
   * Search jobs using the guest API (no authentication required).
   * This is the most reliable method and doesn't require session cookies.
   */
  async searchGuest(params: JobsSearchParams, signal?: AbortSignal): Promise<JobPosting[]> {
    const {
      keywords,
      location,
      start = 0,
      dateFilter,
      jobType,
      workType,
      experienceLevel,
      easyApply,
      activelyHiring,
      verified,
      sortBy,
    } = params;

    const seen = new Set<string>();
    const out: JobPosting[] = [];
    const now = Date.now();

    // Map date filter to LinkedIn f_TPR parameter
    let f_TPR = '';
    if (dateFilter === '30m') f_TPR = 'r1800';
    else if (dateFilter === '1h') f_TPR = 'r3600';
    else if (dateFilter === '2h') f_TPR = 'r7200';
    else if (dateFilter === '4h') f_TPR = 'r14400';
    else if (dateFilter === '8h') f_TPR = 'r28800';
    else if (dateFilter === '24h') f_TPR = 'r86400';
    else if (dateFilter === 'week') f_TPR = 'r604800';
    else if (dateFilter === 'month') f_TPR = 'r2592000';

    // Build URL with all available filters
    const url =
      `https://www.linkedin.com/jobs-guest/jobs/api/seeMoreJobPostings/search` +
      `?keywords=${encodeURIComponent(keywords)}` +
      (location ? `&location=${encodeURIComponent(location)}` : '') +
      `&start=${start}` +
      (jobType ? `&f_JT=${jobType}` : '') + // omit filter when not specified — don't default to full-time only
      (f_TPR ? `&f_TPR=${f_TPR}` : '') +
      (workType ? `&f_WT=${workType}` : '') +
      (experienceLevel ? `&f_E=${experienceLevel}` : '') +
      (easyApply ? `&f_EA=true` : '') +
      (activelyHiring ? `&f_AL=true` : '') +
      (verified ? `&f_VJ=true` : '') +
      (sortBy ? `&sortBy=${sortBy}` : '');

    logger.info({ url }, 'Fetching guest API');

    const html = await this.client.fetchHtml(url, signal);
    logger.info(
      { htmlLength: html.length, htmlPreview: html.substring(0, 500) },
      'Guest API response'
    );

    const $ = load(html);

    const cards = $('li').toArray();
    logger.info({ cardsCount: cards.length }, 'Found li elements');

    if (cards.length === 0) {
      logger.error(
        {
          url,
          htmlLength: html.length,
          htmlSample: html.substring(0, 2000),
          // Detect common failure signatures
          isAuthWall: html.includes('authwall') || html.includes('auth-wall'),
          isCaptcha: html.includes('captcha') || html.includes('CAPTCHA'),
          isChallenge: html.includes('challenge') || html.includes('bot'),
          isRedirect: html.includes('<meta http-equiv="refresh"'),
          bodySnippet: html
            .replace(/<[^>]+>/g, ' ')
            .replace(/\s+/g, ' ')
            .trim()
            .substring(0, 500),
        },
        'Guest API returned 0 job cards — possible block or HTML structure change'
      );
      return [];
    }

    for (const li of cards) {
      if (signal?.aborted) break;
      const $li = $(li);
      const link =
        $li.find('a.base-card__full-link, a.base-search-card__link').first().attr('href') ?? '';
      // The data-entity-urn is on the div inside the li, not on the li itself
      const entityUrn = $li.find('[data-entity-urn]').first().attr('data-entity-urn');
      const id = entityUrn?.split(':').pop();
      if (!id || seen.has(id)) continue;
      seen.add(id);

      const title = $li
        .find('.base-search-card__title, .job-card-container__title')
        .first()
        .text()
        .trim();
      const company = $li
        .find('.base-search-card__subtitle, .job-card-container__subtitle')
        .first()
        .text()
        .trim();
      const loc = $li
        .find('.job-search-card__location, .job-card-container__location')
        .first()
        .text()
        .trim();
      const dateAttr = $li.find('time').first().attr('datetime');

      // Stream job immediately without description for faster UX
      const posting: JobPosting = {
        id: `linkedin:${id}`,
        source: 'linkedin',
        externalId: id,
        url: link.split('?')[0] ?? '',
        title,
        company,
        location: loc,
        description: '', // Will be filled in background
        capturedAt: now,
        ...(dateAttr ? { postedAt: Date.parse(dateAttr) } : {}),
      };
      out.push(posting);

      // Fetch description in background (non-blocking)
      this.fetchJobDescription(id, signal)
        .then((description) => {
          posting.description = description;
        })
        .catch(() => {
          // Description fetch failed, leave empty
        });
    }

    logger.debug({ count: out.length }, 'Fetched jobs');
    return out;
  }

  /**
   * Search jobs with pagination support.
   * Automatically handles multiple pages based on the pages parameter.
   */
  async searchPaginated(
    params: JobsSearchParams,
    pages: number,
    signal?: AbortSignal,
    onProgress?: (progress: number) => void,
    onItem?: (item: JobPosting) => void
  ): Promise<JobPosting[]> {
    const authenticated = this.client.hasSession();
    const maxPages = Math.min(Math.max(pages, 1), 10);
    const allJobs: JobPosting[] = [];
    const seen = new Set<string>();

    logger.info(
      { keywords: params.keywords, pages: maxPages, authenticated },
      'Starting paginated search'
    );

    for (let page = 0; page < maxPages; page++) {
      if (signal?.aborted) break;

      const start = page * PAGE_SIZE;
      logger.debug({ page, start }, 'Fetching page');
      const jobs = await this.searchGuest({ ...params, start }, signal);

      logger.debug({ page, jobsCount: jobs.length }, 'Page fetched');

      // Filter duplicates and stream each job
      for (const job of jobs) {
        if (!seen.has(job.externalId ?? job.id)) {
          seen.add(job.externalId ?? job.id);
          allJobs.push(job);
          // Stream job to UI for immediate display
          if (onItem) onItem(job);
        }
      }

      // If no jobs returned, we've reached the end
      if (jobs.length === 0) {
        logger.debug(`No jobs on page ${page + 1}, stopping pagination`);
        break;
      }

      // Add delay between pages to be polite
      if (page < maxPages - 1) {
        await this.delay(500 + Math.random() * 500);
      }
    }

    if (onProgress) {
      onProgress(1);
    }

    logger.info({ totalJobs: allJobs.length }, 'Paginated search complete');
    return allJobs;
  }

  /**
   * Fetch job description from the detail page.
   * Works for both guest and authenticated requests.
   */
  private async fetchJobDescription(id: string, signal?: AbortSignal): Promise<string> {
    try {
      const url = `https://www.linkedin.com/jobs-guest/jobs/api/jobPosting/${id}`;
      const html = await this.client.fetchHtml(url, signal);
      const $ = load(html);
      const htmlContent =
        $('.show-more-less-html__markup').html() ?? $('.description__text').html() ?? '';
      return stripHtml(htmlContent);
    } catch (error) {
      logger.warn({ error, id }, 'Failed to fetch job description');
      return '';
    }
  }

  /**
   * Delay helper for rate limiting.
   */
  private delay(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }

  /**
   * Update the HTTP client with new session data.
   */
  updateSession(sessionData: LinkedInSessionData): void {
    this.client.updateSession(sessionData);
  }
}
