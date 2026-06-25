import { invoke } from '@tauri-apps/api/core';

import type { GitHubRepo } from '@ajh/shared/ipc';

/** Raw command envelope: `{ repos }` on success, `{ error }` on failure. */
type ImportReposResult = { repos: GitHubRepo[] } | { error: string };

export const github = {
  importRepos: async (input: string): Promise<GitHubRepo[]> => {
    const result = await invoke<ImportReposResult>('github_import_repos', { input });
    // A real backend always returns `{ repos }` or `{ error }`. Surface the error
    // envelope AND any unexpected/malformed shape (missing command, capability
    // denial, serialization failure → undefined) as a thrown error — never mask a
    // failure as a "valid empty list", which would show the user "no repos"
    // instead of the actual problem.
    if (result && 'error' in result) {
      throw new Error(result.error);
    }
    if (!result || !Array.isArray((result as { repos?: unknown }).repos)) {
      throw new Error('GitHub import failed: unexpected response');
    }
    return result.repos;
  },
};
