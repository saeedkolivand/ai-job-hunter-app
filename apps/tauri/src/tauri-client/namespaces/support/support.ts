import { invoke } from '@tauri-apps/api/core';

export const support = {
  exportDiagnostics: () => invoke('support_export_diagnostics'),
  reloadAiRuntime: () => invoke('support_reload_ai_runtime'),
  unloadAllModels: () => invoke('support_unload_all_models'),
  resetModelConfiguration: () => invoke('support_reset_model_configuration'),
  rebuildVectorIndexes: () => invoke('support_rebuild_vector_indexes'),
  clearEmbeddingsCache: () => invoke('support_clear_embeddings_cache'),
  resetVectorDatabase: () => invoke('support_reset_vector_database'),
  clearOcrCache: () => invoke('support_clear_ocr_cache'),
  reindexAllDocuments: () => invoke('support_reindex_all_documents'),
  resetAllSessions: () => invoke('support_reset_all_sessions'),
  clearScrapingQueue: () => invoke('support_clear_scraping_queue'),
  copyEnvironmentDetails: () => invoke('support_copy_environment_details'),
  copyAppVersion: () => invoke('support_copy_app_version'),
  copySystemInfo: () => invoke('support_copy_system_info'),
};
