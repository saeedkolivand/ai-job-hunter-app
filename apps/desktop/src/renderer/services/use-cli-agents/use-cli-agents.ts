import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { CliAgentInstallResult } from '@ajh/shared/ipc';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';

/** Install status for every CLI agent (+ npm availability) — #22. */
export const useCliAgents = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.cliAgents.all,
    queryFn: () => api.cliAgents.status(),
  });
};

/**
 * One-click install a CLI agent via the shell plugin (capability-allowlisted).
 * On success it re-detects (busting the Rust cache) and invalidates the status
 * query so the panel flips to "installed" immediately. Pass `onOutput` to stream
 * the installer log and `signal` to cancel.
 */
export const useInstallCliAgent = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (opts: {
      commandName: string;
      args: string[];
      onOutput?: (line: string) => void;
      signal?: AbortSignal;
    }): Promise<CliAgentInstallResult> => api.cliAgents.install(opts),
    onSuccess: async () => {
      await api.cliAgents.redetect().catch(() => undefined);
      void qc.invalidateQueries({ queryKey: keys.cliAgents.all });
    },
  });
};
