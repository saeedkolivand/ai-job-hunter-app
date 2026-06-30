import { invoke } from '@tauri-apps/api/core';

import type { GitHubRepo } from '@ajh/shared/ipc';

/** Raw command envelope: `{ repos }` on success, `{ error }` on failure. */
type ImportReposResult = { repos: GitHubRepo[] } | { error: string };

export const github = {
  importRepos: async (input: string): Promise<GitHubRepo[]> => {
    const result: unknown = await invoke<ImportReposResult>('github_import_repos', { input });
    // A real backend always returns `{ repos }` or `{ error }`. Surface the error
    // envelope AND any unexpected/malformed shape (missing command, capability
    // denial, serialization failure → undefined or a bare primitive) as a thrown
    // error — never mask a failure as a "valid empty list", which would show the
    // user "no repos" instead of the actual problem. Guard `typeof === object`
    // before `in`, since `'error' in "oops"` throws a TypeError on a primitive.
    if (typeof result !== 'object' || result === null) {
      throw new Error('GitHub import failed: unexpected response');
    }
    if ('error' in result) {
      throw new Error(String((result as { error: unknown }).error));
    }
    const repos = (result as { repos?: unknown }).repos;
    if (!Array.isArray(repos)) {
      throw new Error('GitHub import failed: unexpected response');
    }
    return repos as GitHubRepo[];
  },
};
