import { useEffect } from 'react';
import { useMutation } from '@tanstack/react-query';

import { useAppClient } from '@/providers/AppClientProvider';

export const useExportDiagnostics = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: (dest: string) => api.support.exportDiagnostics(dest) });
};

export const useReloadAiRuntime = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: () => api.support.reloadAiRuntime() });
};

export const useUnloadAllModels = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: () => api.support.unloadAllModels() });
};

export const useResetModelConfiguration = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: () => api.support.resetModelConfiguration() });
};

export const useRebuildVectorIndexes = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: () => api.support.rebuildVectorIndexes() });
};

export const useClearEmbeddingsCache = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: () => api.support.clearEmbeddingsCache() });
};

export const useResetVectorDatabase = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: () => api.support.resetVectorDatabase() });
};

export const useClearOcrCache = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: () => api.support.clearOcrCache() });
};

export const useReindexAllDocuments = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: () => api.support.reindexAllDocuments() });
};

export const useResetAllSessions = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: () => api.support.resetAllSessions() });
};

export const useClearScrapingQueue = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: () => api.support.clearScrapingQueue() });
};

export const useCopyEnvironmentDetails = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: () => api.support.copyEnvironmentDetails() });
};

export const useCopyAppVersion = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: () => api.support.copyAppVersion() });
};

export const useCopySystemInfo = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: () => api.support.copySystemInfo() });
};

export const useCommandPaletteShortcut = (onOpen: () => void) => {
  const api = useAppClient();
  useEffect(() => {
    const off = api.shortcuts.onCommandPalette(onOpen);
    return () => {
      off?.();
    };
  }, [api, onOpen]);
};
