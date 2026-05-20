/**
 * LinkedIn Provider Implementation
 *
 * Implements the JobProvider interface for LinkedIn using the new HTTP-based architecture.
 */
import type { JobPosting } from '@ajh/shared';
import {
  BaseProvider,
  type ProviderContext,
  type ProviderSearchInput,
  type ProviderConfig,
} from './base.js';
import {
  LinkedInJobsApiClient,
  LinkedInHttpClient,
  LinkedInAuthManager,
  type LinkedInSessionData,
} from '../linkedin/index.js';

const LINKEDIN_CONFIG: ProviderConfig = {
  id: 'linkedin',
  displayName: 'LinkedIn',
  requiresAuth: false, // Works without auth, but better with it
  baseUrl: 'https://www.linkedin.com',
};

export class LinkedInProvider extends BaseProvider {
  readonly config = LINKEDIN_CONFIG;

  private httpClient?: LinkedInHttpClient;
  private apiClient?: LinkedInJobsApiClient;
  private authManager?: LinkedInAuthManager;
  private sessionData?: LinkedInSessionData | null;
  private userDataDir: string;

  constructor(userDataDir: string) {
    super();
    this.userDataDir = userDataDir;
  }

  async initialize(): Promise<void> {
    // Initialize auth manager
    this.authManager = new LinkedInAuthManager({ userDataDir: this.userDataDir });

    // Try to load existing session
    if (await this.authManager.hasValidSession()) {
      this.sessionData = await this.authManager.loadSession();
    }
  }

  async search(input: ProviderSearchInput, ctx: ProviderContext): Promise<JobPosting[]> {
    // Create HTTP client with session if available
    this.httpClient = new LinkedInHttpClient({
      sessionData: this.sessionData || undefined,
      signal: ctx.signal,
    });
    this.apiClient = new LinkedInJobsApiClient(this.httpClient);

    const maxPages = Math.min(Math.max(input.pages, 1), 10);

    return this.apiClient.searchPaginated(
      {
        keywords: input.query,
        location: input.location,
        dateFilter: input.dateFilter,
      },
      maxPages,
      ctx.signal,
      ctx.onProgress
    );
  }

  async hasSession(): Promise<boolean> {
    return this.authManager?.hasValidSession() ?? false;
  }

  async close(): Promise<void> {
    await this.authManager?.close();
  }
}
