import { useMutation } from '@tanstack/react-query';

import type { GitHubRepo } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

/**
 * Fetch a user's public GitHub repos (forks dropped, star-sorted, top 30) for
 * the resume-builder "Import from GitHub" step. A mutation, not a query: the
 * username is user-typed input, not a stable cache key, and the call only runs
 * when the user submits. Rejects with the backend's error message on
 * validation / rate-limit / not-found.
 */
export const useGitHubImport = () => {
  const api = useAppClient();
  return useMutation<GitHubRepo[], Error, string>({
    mutationFn: (input: string) => api.github.importRepos(input),
  });
};
