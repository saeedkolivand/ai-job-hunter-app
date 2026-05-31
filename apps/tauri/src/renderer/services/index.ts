/**
 * Service layer — React Query hooks for every IPC namespace.
 *
 * This is the Ports & Adapters boundary between the renderer UI and
 * the Electron main process. Components never call window.api.* directly;
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
export * from './use-apply';
export * from './use-autopilot';
export * from './use-boards';
export * from './use-contact-profile';
export * from './use-conversations';
export * from './use-credentials';
export * from './use-data';
export * from './use-documents';
export * from './use-job-preferences';
export * from './use-jobs';
export * from './use-match';
export * from './use-postings';
export * from './use-privacy';
export * from './use-profile-import';
export * from './use-search';
export * from './use-support';
export * from './use-system';
export * from './use-system-resources';
