// Minimal shapes of the GitHub REST payloads /mission-control reads. Only the
// fields we actually use are typed; everything else on the wire is ignored.

export interface GhRelease {
  tag_name: string;
  name: string | null;
  published_at: string | null;
  html_url: string;
  assets: { name: string; download_count: number }[];
}

export interface GhCommit {
  sha: string;
  commit: { message: string; author: { date: string | null } | null };
  author: { login: string } | null;
}

export interface GhPull {
  number: number;
  title: string;
  html_url: string;
  draft: boolean;
  created_at: string;
  updated_at: string;
  merged_at: string | null;
  closed_at: string | null;
  requested_reviewers: unknown[];
  user: { login: string } | null;
}

export interface GhIssue {
  number: number;
  title: string;
  html_url: string;
  state: string;
  created_at: string;
  updated_at: string;
  closed_at: string | null;
  comments: number;
  labels: ({ name: string } | string)[];
  // Present on the /issues endpoint when the "issue" is really a pull request.
  pull_request?: unknown;
  user: { login: string } | null;
}

export interface GhWorkflowRun {
  name: string;
  path: string;
  head_branch: string | null;
  status: string;
  conclusion: string | null;
  created_at: string;
  updated_at: string;
  run_started_at: string | null;
  html_url: string;
  event: string;
  id: number;
}

export interface GhRepo {
  stargazers_count: number;
  forks_count: number;
  subscribers_count: number;
  open_issues_count: number;
}
