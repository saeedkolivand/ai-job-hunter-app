import { MC_CONFIG } from './config';
import { ghGet, liveOrSnapshot } from './github';
import {
  type BenchRun,
  chaossHealth,
  commitsPerDay,
  commitTypeRatio,
  criticalIssueCount,
  DAY_MS,
  latestGatingConclusion,
  medianLeadTimeHours,
  needsAttention,
  releasesPerWeek,
  staleCount,
  summarizeOpenPulls,
  totalReleaseDownloads,
  workflowHealth,
} from './metrics';
import type { GhCommit, GhIssue, GhPull, GhRelease, GhRepo, GhWorkflowRun } from './types';
import { computeVerdict, isRedConclusion } from './verdict';

// Pure repo → dashboard reduction. Fetches the raw GitHub payloads (or their
// nightly snapshot equivalents) and turns them into the shape every
// /mission-control section renders from. Zero React — MissionControl.tsx owns
// state/effects/composition only.

export interface RepoData {
  repo: GhRepo | null;
  releases: GhRelease[];
  commits: GhCommit[];
  openPulls: GhPull[];
  closedPulls: GhPull[];
  issues: GhIssue[];
  runs: GhWorkflowRun[];
}

export interface BenchmarkGlobal {
  BENCHMARK_DATA?: { entries?: Record<string, BenchRun[]> };
}

export const fmtInt = (n: number): string => new Intl.NumberFormat().format(Math.round(n));
export const round1 = (n: number): string => (Math.round(n * 10) / 10).toFixed(1);
// null (empty sample) renders as an em dash, never a misleading 0% / 100%.
export const pctOrDash = (n: number | null): string =>
  n === null ? '—' : `${Math.round(n * 100)}%`;

async function safe<T>(promise: Promise<T>, fallback: T): Promise<T> {
  try {
    return await promise;
  } catch {
    return fallback;
  }
}

export async function loadAll(token: string): Promise<RepoData> {
  const src = MC_CONFIG.dataSource;
  const get = <T>(key: string, path: string) =>
    liveOrSnapshot(src, key, () => ghGet<T>(path, token));

  // The repo call is the primary — if it throws (rate limit / network), the
  // error surfaces to the UI rather than silently emptying the dashboard.
  const repo = await get<GhRepo>('repo', '');
  const [releases, commits, openPulls, closedPulls, issues, runsWrap] = await Promise.all([
    safe(get<GhRelease[]>('releases', '/releases?per_page=30'), []),
    safe(get<GhCommit[]>('commits', '/commits?per_page=100'), []),
    safe(get<GhPull[]>('open-pulls', '/pulls?state=open&per_page=50'), []),
    safe(
      get<GhPull[]>('closed-pulls', '/pulls?state=closed&per_page=50&sort=updated&direction=desc'),
      []
    ),
    safe(get<GhIssue[]>('issues', '/issues?state=open&per_page=100'), []),
    safe(get<{ workflow_runs: GhWorkflowRun[] }>('runs', '/actions/runs?per_page=50&branch=main'), {
      workflow_runs: [],
    }),
  ]);

  return { repo, releases, commits, openPulls, closedPulls, issues, runs: runsWrap.workflow_runs };
}

export function buildModel(data: RepoData) {
  const now = Date.now();
  const openPullViews = summarizeOpenPulls(data.openPulls, now);
  const stale = staleCount(openPullViews, MC_CONFIG.staleDays);
  const critical = criticalIssueCount(data.issues, MC_CONFIG.criticalLabels);
  const gatingConclusion = latestGatingConclusion(data.runs, MC_CONFIG.gatingWorkflow);
  const latestRelease = data.releases[0];
  const daysSinceRelease = latestRelease?.published_at
    ? Math.floor((now - Date.parse(latestRelease.published_at)) / DAY_MS)
    : null;

  const failedGatingRun = data.runs.find(
    (r) =>
      r.path.endsWith(MC_CONFIG.gatingWorkflow) &&
      r.head_branch === 'main' &&
      r.status === 'completed' &&
      isRedConclusion(r.conclusion)
  );

  return {
    now,
    verdict: computeVerdict({
      gatingRed: isRedConclusion(gatingConclusion),
      gatingKnown: gatingConclusion !== null,
      criticalIssueCount: critical,
      daysSinceRelease,
      openPrCount: openPullViews.length,
      stalePrCount: stale,
      staleDays: MC_CONFIG.staleDays,
    }),
    delivery: {
      perWeek: releasesPerWeek(data.releases, now),
      leadHours: medianLeadTimeHours(
        data.closedPulls.filter((p) => p.merged_at !== null),
        data.releases
      ),
      commitRatio: commitTypeRatio(data.commits),
      health: workflowHealth(data.runs, MC_CONFIG.gatingWorkflow),
      recentReleases: data.releases.slice(0, 5),
      daysSinceRelease,
    },
    work: {
      openPullViews: openPullViews.slice(0, 12),
      totalOpenPulls: openPullViews.length,
      stale,
      critical,
      attention: needsAttention(data.issues, now, MC_CONFIG.staleDays).slice(0, 8),
      failedGatingRun,
    },
    quality: {
      chaoss: chaossHealth({
        issues: data.issues,
        closedPulls: data.closedPulls,
        releases: data.releases,
        commits: data.commits,
        nowMs: now,
      }),
    },
    community: {
      repo: data.repo,
      downloads: totalReleaseDownloads(data.releases),
      commitActivity: commitsPerDay(data.commits, now, 21),
    },
  };
}

export type Model = ReturnType<typeof buildModel>;
