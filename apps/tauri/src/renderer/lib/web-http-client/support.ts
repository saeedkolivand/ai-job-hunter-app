import { createHttpClientHelpers, type WebHttpClientOptions } from './utils.js';

export function support(opts: WebHttpClientOptions) {
  const { cmd } = createHttpClientHelpers(opts);
  return {
    exportDiagnostics: () => cmd('support', 'exportDiagnostics'),
    reloadAiRuntime: () => cmd('support', 'reloadAiRuntime'),
    unloadAllModels: () => cmd('support', 'unloadAllModels'),
    resetModelConfiguration: () => cmd('support', 'resetModelConfiguration'),
    rebuildVectorIndexes: () => cmd('support', 'rebuildVectorIndexes'),
    clearEmbeddingsCache: () => cmd('support', 'clearEmbeddingsCache'),
    resetVectorDatabase: () => cmd('support', 'resetVectorDatabase'),
    clearOcrCache: () => cmd('support', 'clearOcrCache'),
    reindexAllDocuments: () => cmd('support', 'reindexAllDocuments'),
    resetAllSessions: () => cmd('support', 'resetAllSessions'),
    clearScrapingQueue: () => cmd('support', 'clearScrapingQueue'),
    copyEnvironmentDetails: () => cmd('support', 'copyEnvironmentDetails'),
    copyAppVersion: () => cmd('support', 'copyAppVersion'),
    copySystemInfo: () => cmd('support', 'copySystemInfo'),
  };
}
