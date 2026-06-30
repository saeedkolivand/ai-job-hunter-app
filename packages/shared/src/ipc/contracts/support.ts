export interface SupportContract {
  /** Build and save a redacted diagnostics zip to the caller-supplied path */
  exportDiagnostics(
    dest: string
  ): Promise<{ success: true; path: string } | { success: false; error: string }>;

  /** Reload AI runtime and reload all models */
  reloadAiRuntime(): Promise<{ success: boolean }>;

  /** Unload all currently loaded AI models */
  unloadAllModels(): Promise<{ success: boolean }>;

  /** Reset all model settings to defaults */
  resetModelConfiguration(): Promise<{ success: boolean }>;

  /** Rebuild all vector indexes from scratch */
  rebuildVectorIndexes(): Promise<{ success: boolean; jobId?: string }>;

  /** Remove all cached embeddings */
  clearEmbeddingsCache(): Promise<{ success: boolean }>;

  /** Completely reset the vector database */
  resetVectorDatabase(): Promise<{ success: boolean }>;

  /** Remove all cached OCR results */
  clearOcrCache(): Promise<{ success: boolean }>;

  /** Re-index all documents in the database */
  reindexAllDocuments(): Promise<{ success: boolean; jobId?: string }>;

  /** Clear all scraping sessions and cookies */
  resetAllSessions(): Promise<{ success: boolean }>;

  /** Clear all pending scrape jobs */
  clearScrapingQueue(): Promise<{ success: boolean }>;

  /** Copy environment details to clipboard */
  copyEnvironmentDetails(): Promise<{ success: boolean }>;

  /** Copy app version to clipboard */
  copyAppVersion(): Promise<{ success: boolean }>;

  /** Copy system info to clipboard */
  copySystemInfo(): Promise<{ success: boolean }>;
}

export const SUPPORT_CHANNELS = {
  exportDiagnostics: 'support:exportDiagnostics',
  reloadAiRuntime: 'support:reloadAiRuntime',
  unloadAllModels: 'support:unloadAllModels',
  resetModelConfiguration: 'support:resetModelConfiguration',
  rebuildVectorIndexes: 'support:rebuildVectorIndexes',
  clearEmbeddingsCache: 'support:clearEmbeddingsCache',
  resetVectorDatabase: 'support:resetVectorDatabase',
  clearOcrCache: 'support:clearOcrCache',
  reindexAllDocuments: 'support:reindexAllDocuments',
  resetAllSessions: 'support:resetAllSessions',
  clearScrapingQueue: 'support:clearScrapingQueue',
  copyEnvironmentDetails: 'support:copyEnvironmentDetails',
  copyAppVersion: 'support:copyAppVersion',
  copySystemInfo: 'support:copySystemInfo',
} as const;
