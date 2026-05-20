import { useEffect } from 'react';
import { useMutation } from '@tanstack/react-query';

export const useExportDiagnostics = () =>
  useMutation({ mutationFn: () => window.api.support.exportDiagnostics() });

export const useReloadAiRuntime = () =>
  useMutation({ mutationFn: () => window.api.support.reloadAiRuntime() });

export const useUnloadAllModels = () =>
  useMutation({ mutationFn: () => window.api.support.unloadAllModels() });

export const useResetModelConfiguration = () =>
  useMutation({ mutationFn: () => window.api.support.resetModelConfiguration() });

export const useRebuildVectorIndexes = () =>
  useMutation({ mutationFn: () => window.api.support.rebuildVectorIndexes() });

export const useClearEmbeddingsCache = () =>
  useMutation({ mutationFn: () => window.api.support.clearEmbeddingsCache() });

export const useResetVectorDatabase = () =>
  useMutation({ mutationFn: () => window.api.support.resetVectorDatabase() });

export const useClearOcrCache = () =>
  useMutation({ mutationFn: () => window.api.support.clearOcrCache() });

export const useReindexAllDocuments = () =>
  useMutation({ mutationFn: () => window.api.support.reindexAllDocuments() });

export const useResetAllSessions = () =>
  useMutation({ mutationFn: () => window.api.support.resetAllSessions() });

export const useClearScrapingQueue = () =>
  useMutation({ mutationFn: () => window.api.support.clearScrapingQueue() });

export const useCopyEnvironmentDetails = () =>
  useMutation({ mutationFn: () => window.api.support.copyEnvironmentDetails() });

export const useCopyAppVersion = () =>
  useMutation({ mutationFn: () => window.api.support.copyAppVersion() });

export const useCopySystemInfo = () =>
  useMutation({ mutationFn: () => window.api.support.copySystemInfo() });

/** Subscribe to the global Cmd/Ctrl+K shortcut from the main process. */
export const useCommandPaletteShortcut = (onOpen: () => void) => {
  useEffect(() => {
    const off = window.api?.shortcuts.onCommandPalette(onOpen);
    return () => {
      off?.();
    };
  }, [onOpen]);
};
