import { useMutation } from '@tanstack/react-query';

import { useAppClient } from '@/providers/AppClientProvider';

export const useExportDiagnostics = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: (dest: string) => api.support.exportDiagnostics(dest) });
};
