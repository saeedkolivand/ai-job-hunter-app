import { useEffect } from 'react';
import { useMutation } from '@tanstack/react-query';

import { useAppClient } from '@/providers/AppClientProvider';

export const useExportDiagnostics = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: (dest: string) => api.support.exportDiagnostics(dest) });
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
