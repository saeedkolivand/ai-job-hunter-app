/**
 * GitHub repos import — fetch a user's public repos for the resume-builder
 * "Import from GitHub" projects step. The backend extracts + validates the
 * username (bare name or `github.com/<user>` URL), drops forks, sorts by stars,
 * and caps to the top 30. Fields are camelCase to match the Rust output struct.
 */

/** A single public GitHub repo offered to the candidate for import. */
export interface GitHubRepo {
  name: string;
  /** Omitted by the backend when absent (serde `skip_serializing_if`). */
  description?: string;
  /** Canonical repo URL — kept verbatim; the AI step never rewrites it. */
  htmlUrl: string;
  language?: string;
  topics: string[];
  /** `stargazers_count` from the GitHub API. */
  stars: number;
  pushedAt?: string;
}

export interface GitHubContract {
  /**
   * Fetch a user's public repos. `input` is a bare username or a
   * `github.com/<user>` URL. Resolves to the repo list (the `{ repos }`
   * envelope is unwrapped in the client layer); rejects on validation /
   * rate-limit / not-found errors.
   */
  importRepos(input: string): Promise<GitHubRepo[]>;
}

export const GITHUB_CHANNELS = {
  importRepos: 'github_import_repos',
} as const;
