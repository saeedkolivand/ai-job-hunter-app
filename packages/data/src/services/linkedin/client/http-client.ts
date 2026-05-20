/**
 * LinkedIn HTTP Client
 *
 * HTTP client with browser-mimicking headers for LinkedIn API requests.
 * Reuses session cookies from Playwright authentication.
 * Uses undici for faster HTTP requests.
 */
import { Pool, request } from 'undici';
import { gunzipSync } from 'zlib';

import type { LinkedInSessionData } from '../session/store.js';
import { linkedinRateLimiter } from './rate-limiter.js';

// Create connection pool for better performance
const pool = new Pool('https://www.linkedin.com', {
  connections: 10,
  pipelining: 1,
  keepAliveTimeout: 60000,
  keepAliveMaxTimeout: 300000,
});

export interface LinkedInHttpClientOptions {
  sessionData?: LinkedInSessionData;
  signal?: AbortSignal;
}

export class LinkedInHttpClient {
  private sessionData?: LinkedInSessionData;
  private userAgent =
    'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36';

  constructor(options: LinkedInHttpClientOptions = {}) {
    this.sessionData = options.sessionData;
  }

  /**
   * Update session data (cookies).
   */
  updateSession(sessionData: LinkedInSessionData): void {
    this.sessionData = sessionData;
  }

  /**
   * Get default headers that mimic a real browser.
   */
  private getDefaultHeaders(): Record<string, string> {
    const headers: Record<string, string> = {
      'User-Agent': this.userAgent,
      Accept: 'text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8',
      'Accept-Language': 'en-US,en;q=0.9,de;q=0.8',
      'Accept-Encoding': 'gzip, deflate, br',
      DNT: '1',
      Connection: 'keep-alive',
      'Upgrade-Insecure-Requests': '1',
      'Sec-Fetch-Dest': 'document',
      'Sec-Fetch-Mode': 'navigate',
      'Sec-Fetch-Site': 'none',
      'Sec-Fetch-User': '?1',
      'Cache-Control': 'max-age=0',
    };

    // Add session cookies if available
    if (this.sessionData?.li_at) {
      headers['Cookie'] = `li_at=${this.sessionData.li_at}`;
      if (this.sessionData.JSESSIONID) {
        headers['Cookie'] += `; JSESSIONID=${this.sessionData.JSESSIONID}`;
      }
    }

    // Add CSRF token if available
    if (this.sessionData?.csrfToken) {
      headers['X-CSRF-Token'] = this.sessionData.csrfToken;
      headers['csrf-token'] = this.sessionData.csrfToken;
    }

    return headers;
  }

  /**
   * Make a GET request with rate limiting and retry logic.
   */
  async get<T>(url: string, signal?: AbortSignal): Promise<T> {
    return linkedinRateLimiter.execute(async () => {
      const response = await request(url, {
        method: 'GET',
        headers: this.getDefaultHeaders(),
        signal,
        dispatcher: pool,
      });

      const buffer = await response.body.arrayBuffer();
      const bodyBuffer = Buffer.from(buffer);

      // Decompress gzip if needed
      let body: string;
      if (bodyBuffer[0] === 0x1f && bodyBuffer[1] === 0x8b) {
        // Gzip magic number
        body = gunzipSync(bodyBuffer).toString('utf-8');
      } else {
        body = bodyBuffer.toString('utf-8');
      }

      if (response.statusCode !== 200) {
        throw new Error(`HTTP ${response.statusCode}: Request failed`);
      }

      return body as T;
    }, signal);
  }

  /**
   * Make a POST request with rate limiting and retry logic.
   */
  async post<T>(url: string, data?: Record<string, unknown>, signal?: AbortSignal): Promise<T> {
    return linkedinRateLimiter.execute(async () => {
      const response = await request(url, {
        method: 'POST',
        headers: {
          ...this.getDefaultHeaders(),
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(data),
        signal,
        dispatcher: pool,
      });

      const body = await response.body.text();

      if (response.statusCode !== 200) {
        throw new Error(`HTTP ${response.statusCode}: Request failed`);
      }

      return body as T;
    }, signal);
  }

  /**
   * Fetch HTML content from a URL.
   */
  async fetchHtml(url: string, signal?: AbortSignal): Promise<string> {
    return this.get<string>(url, signal);
  }

  /**
   * Fetch JSON data from a URL.
   */
  async fetchJson<T>(url: string, signal?: AbortSignal): Promise<T> {
    const body = await this.get<string>(url, signal);
    return JSON.parse(body) as T;
  }

  /**
   * Check if the client has an active session.
   */
  hasSession(): boolean {
    return !!this.sessionData?.li_at;
  }
}
