/**
 * TauriInvokeClient — implements AppClient using Tauri v2 invoke + listen.
 *
 * Used by apps/tauri/src/main.tsx instead of the Electron window.api bridge.
 * The shape is structurally identical to the Electron client so all service
 * hooks work without modification.
 *
 * Event subscriptions: Tauri's listen() is async. We return a sync cleanup
 * function that cancels the listener once the promise resolves. This matches
 * the Electron preload pattern.
 *
 * Unimplemented commands return stubs (null / []) so the UI degrades
 * gracefully while parity is being built up incrementally.
 */
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import type { AppClient } from '@/lib/app-client';

// Registers a Tauri event listener and returns a sync unsubscribe handle.
function asyncUnsub(setup: () => Promise<() => void>): () => void {
  let cancel: (() => void) | null = null;
  let cancelled = false;
  setup()
    .then((fn) => {
      if (cancelled) fn();
      else cancel = fn;
    })
    .catch(console.error);
  return () => {
    cancelled = true;
    cancel?.();
  };
}

const NOT_AVAILABLE = Promise.resolve(null) as Promise<unknown>;
const EMPTY_LIST = Promise.resolve([]) as Promise<unknown>;

export function createTauriInvokeClient(): AppClient {
  return {
    system: {
      health: () => invoke('system_health'),
      getVersion: () => invoke('system_get_version'),
      getLocale: () => invoke('system_get_locale'),
      setLocale: (locale) => invoke('system_set_locale', { locale }),
      getPlatform: () => invoke('system_get_platform'),
      openExternal: (url) => invoke('system_open_external', { url }),
      setPerformanceMode: (mode) => invoke('system_set_performance_mode', { mode }),
      getMetrics: () => invoke('system_get_metrics'),
    },

    jobs: {
      list: () => invoke('jobs_list'),
      get: (jobId) => invoke('jobs_get', { jobId }),
      cancel: (jobId) => invoke('jobs_cancel', { jobId }),
      retry: (jobId) => invoke('jobs_retry', { jobId }),
      onEvent: (handler) =>
        asyncUnsub(() => listen<unknown>('jobs:event', (e) => handler(e.payload))),
    },

    ai: {
      generate: (req) => invoke('ai_generate', { req }),
      listModels: () => invoke('ai_list_models'),
      pullModel: (model) => invoke('ai_pull_model', { model }),
      unloadModel: (model) => invoke('ai_unload_model', { model }),
      embed: (req) => invoke('ai_embed', { req }),
      onStream: (handler) =>
        asyncUnsub(() => listen<unknown>('ai:stream', (e) => handler(e.payload))),
    },

    documents: {
      list: () => invoke('documents_list'),
      import: (req) => invoke('documents_import', { req }),
      remove: (id) => invoke('documents_remove', { id }),
    },

    search: {
      hybrid: (req) => invoke('search_hybrid', { req }),
    },

    scrape: {
      board: (req) => invoke('scrape_board', { req }),
      url: (req) => invoke('scrape_url', { req }),
      persistJob: (req) => invoke('scrape_persist_job', { req }),
      listPostings: () => invoke('scrape_list_postings'),
      clearPostings: () => invoke('scrape_clear_postings'),
      listInteractions: (filter) => invoke('scrape_list_interactions', { filter }),
      exportData: () => invoke('scrape_export_data'),
      importData: () => invoke('scrape_import_data'),
    },

    match: {
      resume: (req) => invoke('match_resume', { req }),
    },

    credentials: {
      available: () => invoke('credentials_available'),
      list: () => invoke('credentials_list'),
      set: (req) => invoke('credentials_set', { req }),
      remove: (boardId) => invoke('credentials_remove', { boardId }),
    },

    linkedin: {
      connect: () => invoke('linkedin_connect'),
      disconnect: () => invoke('linkedin_disconnect'),
      getStatus: () => invoke('linkedin_get_status'),
    },

    boards: {
      connect: (boardId) => invoke('boards_connect', { boardId }),
      disconnect: (boardId) => invoke('boards_disconnect', { boardId }),
      getStatus: (boardId) => invoke('boards_get_status', { boardId }),
    },

    privacy: {
      signOutAll: () => invoke('privacy_sign_out_all'),
      clearInteractions: () => invoke('privacy_clear_interactions'),
    },

    apply: {
      start: (req) => invoke('apply_start', { req }),
      catalog: () => invoke('apply_catalog'),
    },

    updater: {
      check: () => NOT_AVAILABLE,
      download: () => NOT_AVAILABLE,
      install: () => NOT_AVAILABLE,
      onStatus: (_handler) => () => {
        /* Tauri updater plugin wired separately */
      },
    },

    shortcuts: {
      onCommandPalette: (handler) =>
        asyncUnsub(() => listen('shortcut:command-palette', () => handler())),
    },

    resume: {
      extractText: (req) => invoke('resume_extract_text', { req }),
    },

    support: {
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
    },

    conversations: {
      getOrCreateConversation: () => invoke('conversations_get_or_create'),
      loadMessages: (conversationId) => invoke('conversations_load_messages', { conversationId }),
      saveMessage: (req) => invoke('conversations_save_message', { req }),
    },

    autopilot: {
      list: () => EMPTY_LIST,
      get: (_id) => NOT_AVAILABLE,
      create: (_req) => NOT_AVAILABLE,
      update: (_id, _req) => NOT_AVAILABLE,
      remove: (_id) => NOT_AVAILABLE,
      run: (_id) => NOT_AVAILABLE,
      pause: (_id) => NOT_AVAILABLE,
      resume: (_id) => NOT_AVAILABLE,
    },

    dialog: {
      openFiles: (opts) => invoke('dialog_open_files', opts ?? {}),
    },
  } satisfies AppClient;
}
