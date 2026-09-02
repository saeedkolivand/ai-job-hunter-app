import { useMutation } from '@tanstack/react-query';

import type { HelpSearchRequest } from '@ajh/shared/schemas';

import { useAppClient } from '@/providers/AppClientProvider';

/**
 * Rank the help corpus against one user question (`help:search`).
 *
 * A mutation, not a query, and deliberately without a query key: every call
 * carries a different question, the reply is only ever consumed once by the
 * turn that asked, and caching it would mean holding the user's typed
 * questions in the React Query cache for the rest of the session. Same shape
 * as `useExportDiagnostics` — a one-shot backend action, not shared state.
 */
export const useHelpSearch = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: (req: HelpSearchRequest) => api.help.search(req) });
};
