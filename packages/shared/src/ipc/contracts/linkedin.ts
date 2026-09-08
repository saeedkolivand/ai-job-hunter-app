import type { CookieImportResult } from './boards';

export interface LinkedinContract {
  // `connect`/`disconnect`/`getStatus`/`importCookies` dispatch the SAME Tauri command as their
  // `BoardsContract` namesake (`boardId: 'linkedin'` baked in). The two contracts' TSDoc wording
  // for those members is free to differ (issue #1183 F2) — the agent-CLI catalogue generator's
  // duplicate-call-site guard (`gen-agent-catalogue.ts`'s `mergeCatalogueEntry`) only enforces
  // that two namespaces sharing a dispatched command agree on its ARGUMENT shape (the contract
  // `check_input` actually validates); the published DESCRIPTION is keyed per namespace, so this
  // file keeps its own LinkedIn-specific wording without failing codegen.

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

  /** Import an existing LinkedIn session from the installed browser's cookie store. */
  importCookies(): Promise<CookieImportResult>;
}

export const LINKEDIN_CHANNELS = {
  connect: 'linkedin:connect',
  disconnect: 'linkedin:disconnect',
  getStatus: 'linkedin:getStatus',
  importProfileFromUrl: 'linkedin:importProfileFromUrl',
  importCookies: 'linkedin:importCookies',
} as const;
