/**
 * Service layer — React Query hooks for every IPC namespace.
 *
 * This is the Ports & Adapters boundary between the renderer UI and
 * the Tauri shell. Components never call window.api.* directly;
 * they use these hooks instead.
 *
 * Benefits:
 *   - Caching + deduplication via React Query
 *   - Consistent loading / error state
 *   - Automatic cache invalidation across the app
 *   - IPC calls testable via query-client mock
 *   - Library/transport can be swapped in one layer
 */
export * from './query-client';
export * from './use-ai';
export * from './use-ai-provider';
export * from './use-applications';
export * from './use-autopilot';
export * from './use-boards';
export * from './use-cli-agents';
export * from './use-company-logo';
export * from './use-contact-profile';
export * from './use-credentials';
export * from './use-data';
export * from './use-documents';
export * from './use-extension-bridge';
export * from './use-geocode';
export * from './use-github-import';
export * from './use-job-preferences';
export * from './use-jobs';
export * from './use-match';
export * from './use-menu';
export * from './use-notifications';
export * from './use-postings';
export * from './use-privacy';
export * from './use-profile-import';
export * from './use-referrals';
export * from './use-support';
export * from './use-system';
export * from './use-system-resources';
export * from './use-window-controls';
