import type { DataSource } from './github';

const OWNER = 'saeedkolivand';
const REPO = 'ai-job-hunter-app';

// Static configuration for the /mission-control dashboard. All data is fetched
// client-side from the anonymous GitHub REST API; a token (see pat.ts) only
// raises the rate limit and unlocks the safe-tier write actions.
export const MC_CONFIG = {
  owner: OWNER,
  repo: REPO,
  // Token is NEVER interpolated into any URL — it rides the Authorization header
  // only (see github.ts::authHeaders). This base contains only the public slug.
  apiBase: `https://api.github.com/repos/${OWNER}/${REPO}`,
  htmlBase: `https://github.com/${OWNER}/${REPO}`,
  // Same-origin benchmark series already published by the benchmark workflow.
  benchmarksSrc: '/benchmarks/data.js',
  cachePrefix: 'ajh-mc-cache:',
  tokenKey: 'ajh-mc-gh-token',
  cacheTtlMs: 4 * 60_000,
  // A PR older than this (open, not draft) is "gathering dust".
  staleDays: 14,
  // Workflow file names used by the write actions + workflow-health read.
  gatingWorkflow: 'ci-pipeline.yml',
  releaseWorkflow: 'release.yml',
  pagesWorkflow: 'pages.yml',
  criticalLabels: ['critical', 'p0', 'priority: critical', 'severity: critical'],
  // The single data-source seam (ADR-0018 PR4): flip `mode` to 'snapshot' with a
  // `snapshotBase` and every widget reads pre-baked nightly JSON instead of the
  // live API — config change, not a rewrite. Ships 'live' in PR2.
  dataSource: { mode: 'live' } as DataSource,
} as const;
