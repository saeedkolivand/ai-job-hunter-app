/**
 * Base Provider Interface
 *
 * Abstraction layer for job board providers.
 * All job boards (LinkedIn, Indeed, Stepstone, Xing, etc.) implement this interface.
 */
import type { JobPosting } from '@ajh/shared';

export interface ProviderSearchInput {
  query: string;
  location?: string;
  pages: number;
  dateFilter?: string;
}

export interface ProviderContext {
  signal: AbortSignal;
  onProgress?: (progress: number) => void;
  onItem?: (item: JobPosting) => void | Promise<void>;
}

export interface ProviderConfig {
  id: string;
  displayName: string;
  requiresAuth: boolean;
  baseUrl: string;
}

export interface JobProvider {
  readonly config: ProviderConfig;

  /**
   * Search for jobs on this provider.
   */
  search(input: ProviderSearchInput, ctx: ProviderContext): Promise<JobPosting[]>;

  /**
   * Get a specific job posting from a URL.
   */
  fromUrl?(url: string, ctx: ProviderContext): Promise<JobPosting | null>;

  /**
   * Check if the provider has an active authenticated session.
   */
  hasSession?(): boolean | Promise<boolean>;

  /**
   * Initialize the provider (load sessions, etc.).
   */
  initialize?(): Promise<void>;

  /**
   * Cleanup resources.
   */
  close?(): Promise<void>;
}

export abstract class BaseProvider implements JobProvider {
  abstract readonly config: ProviderConfig;

  abstract search(input: ProviderSearchInput, ctx: ProviderContext): Promise<JobPosting[]>;

  protected makeId(externalId: string): string {
    return `${this.config.id}:${externalId}`;
  }
}
