/**
 * Rate Limiter with Exponential Backoff
 *
 * Implements rate limiting for API requests to avoid being blocked.
 * Uses exponential backoff with randomized jitter for retries.
 */
import { createLogger } from '@ajh/core';

const logger = createLogger('linkedin.rate-limiter');

export interface RateLimitOptions {
  maxRequests: number; // Max requests per window
  windowMs: number; // Time window in milliseconds
  maxRetries: number; // Max retry attempts
  initialDelay: number; // Initial delay before first retry (ms)
  maxDelay: number; // Maximum delay between retries (ms)
}

export class RateLimiter {
  private requests: number[] = [];
  private readonly options: Required<RateLimitOptions>;

  constructor(options: Partial<RateLimitOptions> = {}) {
    this.options = {
      maxRequests: options.maxRequests ?? 10,
      windowMs: options.windowMs ?? 60000, // 1 minute
      maxRetries: options.maxRetries ?? 5,
      initialDelay: options.initialDelay ?? 1000,
      maxDelay: options.maxDelay ?? 30000,
    };
  }

  /**
   * Wait if necessary to respect rate limits.
   * Returns immediately if under the limit, otherwise waits.
   */
  async waitForSlot(): Promise<void> {
    const now = Date.now();
    // Remove requests outside the current window
    this.requests = this.requests.filter((t) => now - t < this.options.windowMs);

    if (this.requests.length >= this.options.maxRequests) {
      const oldestRequest = this.requests[0];
      if (oldestRequest) {
        const waitTime = oldestRequest + this.options.windowMs - now;
        if (waitTime > 0) {
          logger.debug({ waitTime }, 'Rate limit reached, waiting');
          await this.delay(waitTime);
        }
      }
    }
  }

  /**
   * Record a request was made.
   */
  recordRequest(): void {
    this.requests.push(Date.now());
  }

  /**
   * Execute a function with rate limiting and retry logic.
   */
  async execute<T>(fn: () => Promise<T>, signal?: AbortSignal): Promise<T> {
    let lastError: Error | null = null;
    let attempt = 0;

    while (attempt <= this.options.maxRetries) {
      if (signal?.aborted) {
        throw new Error('Request aborted');
      }

      await this.waitForSlot();

      try {
        const result = await fn();
        this.recordRequest();
        return result;
      } catch (error) {
        lastError = error as Error;
        attempt++;

        // Don't retry on abort
        if (signal?.aborted) {
          throw lastError;
        }

        // Don't retry on certain errors
        if (!this.shouldRetry(error as Error)) {
          throw lastError;
        }

        if (attempt <= this.options.maxRetries) {
          const delay = this.calculateBackoff(attempt);
          logger.warn({ attempt, delay, error: lastError.message }, 'Request failed, retrying');
          await this.delay(delay);
        }
      }
    }

    throw lastError;
  }

  /**
   * Calculate exponential backoff with jitter.
   */
  private calculateBackoff(attempt: number): number {
    const exponentialDelay = this.options.initialDelay * Math.pow(2, attempt - 1);
    const jitter = Math.random() * 0.3 * exponentialDelay; // 30% jitter
    const delay = Math.min(exponentialDelay + jitter, this.options.maxDelay);
    return Math.floor(delay);
  }

  /**
   * Determine if an error is retryable.
   */
  private shouldRetry(error: Error): boolean {
    const message = error.message.toLowerCase();

    // Retry on rate limiting (429)
    if (message.includes('429') || message.includes('too many requests')) {
      return true;
    }

    // Retry on server errors (5xx)
    if (
      message.includes('500') ||
      message.includes('502') ||
      message.includes('503') ||
      message.includes('504')
    ) {
      return true;
    }

    // Retry on network errors
    if (
      message.includes('network') ||
      message.includes('timeout') ||
      message.includes('econnrefused')
    ) {
      return true;
    }

    return false;
  }

  /**
   * Simple delay helper.
   */
  private delay(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }

  /**
   * Reset the rate limiter state.
   */
  reset(): void {
    this.requests = [];
  }
}

/**
 * Global rate limiter instance for LinkedIn requests.
 */
export const linkedinRateLimiter = new RateLimiter({
  maxRequests: 10,
  windowMs: 60000,
  maxRetries: 5,
  initialDelay: 1000,
  maxDelay: 30000,
});
