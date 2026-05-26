export interface LinkedinContract {
  /** Connect to LinkedIn by launching a browser for manual login. */
  connect(): Promise<{ connected: boolean; accountEmail?: string }>;

  /** Disconnect and clear LinkedIn session. */
  disconnect(): Promise<void>;

  /** Get current LinkedIn session status. */
  getStatus(): Promise<{ connected: boolean; accountEmail?: string; lastConnected?: number }>;

  /** Fetch a LinkedIn profile URL and return extracted resume text. */
  importProfileFromUrl(
    url: string
  ): Promise<{ text: string; name?: string; platform: string } | { error: string }>;
}

export const LINKEDIN_CHANNELS = {
  connect: 'linkedin:connect',
  disconnect: 'linkedin:disconnect',
  getStatus: 'linkedin:getStatus',
} as const;
