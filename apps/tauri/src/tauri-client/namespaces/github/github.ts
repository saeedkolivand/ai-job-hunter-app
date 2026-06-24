import { invoke } from '@tauri-apps/api/core';

import type { GitHubRepo } from '@ajh/shared/ipc';

/** Raw command envelope: `{ repos }` on success, `{ error }` on failure. */
type ImportReposResult = { repos: GitHubRepo[] } | { error: string };

export const github = {
  importRepos: async (input: string): Promise<GitHubRepo[]> => {
    const result = await invoke<ImportReposResult>('github_import_repos', { input });
    if ('error' in result) {
      throw new Error(result.error);
    }
    return result.repos;
  },
};
