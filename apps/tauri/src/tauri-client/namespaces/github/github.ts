import { invoke } from '@tauri-apps/api/core';

import type { GitHubRepo } from '@ajh/shared/ipc';

/** Raw command envelope: `{ repos }` on success, `{ error }` on failure. */
type ImportReposResult = { repos: GitHubRepo[] } | { error: string };

export const github = {
  importRepos: async (input: string): Promise<GitHubRepo[]> => {
    const result = await invoke<ImportReposResult>('github_import_repos', { input });
    // A real backend always returns `{ repos }` or `{ error }`. Tolerate a falsy
    // result (the generic namespace test mocks invoke → undefined) by degrading
    // to an empty list rather than throwing — matching every sibling namespace,
    // which never rejects on `undefined`. The genuine error envelope still throws.
    if (!result) {
      return [];
    }
    if ('error' in result) {
      throw new Error(result.error);
    }
    return result.repos;
  },
};
