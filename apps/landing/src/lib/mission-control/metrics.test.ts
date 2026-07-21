import { describe, expect, it } from 'vitest';

import {
  chaossHealth,
  commitsPerDay,
  commitTypeRatio,
  criticalIssueCount,
  DAY_MS,
  HOUR_MS,
  issuesByLabel,
  latestGatingConclusion,
  median,
  medianLeadTimeHours,
  needsAttention,
  realIssues,
  releasesPerWeek,
  staleCount,
  summarizeBenchmarks,
  summarizeOpenPulls,
  totalReleaseDownloads,
  workflowHealth,
} from './metrics';
import type { GhCommit, GhIssue, GhPull, GhRelease, GhWorkflowRun } from './types';

const NOW = Date.parse('2026-07-20T00:00:00Z');
const iso = (msAgo: number): string => new Date(NOW - msAgo).toISOString();

function release(publishedMsAgo: number, downloads: number[] = []): GhRelease {
  return {
    tag_name: `v${publishedMsAgo}`,
    name: null,
    published_at: iso(publishedMsAgo),
    html_url: 'https://example/release',
    assets: downloads.map((count, index) => ({ name: `asset-${index}`, download_count: count })),
  };
}
function commit(message: string, login: string | null = 'saeed', dateMsAgo = DAY_MS): GhCommit {
  return {
    sha: message.slice(0, 7),
    commit: { message, author: { date: iso(dateMsAgo) } },
    author: login ? { login } : null,
  };
}
function pull(overrides: Partial<GhPull>): GhPull {
  return {
    number: 1,
    title: 'a PR',
    html_url: 'https://example/pr',
    draft: false,
    created_at: iso(DAY_MS),
    updated_at: iso(DAY_MS),
    merged_at: null,
    closed_at: null,
    requested_reviewers: [],
    user: { login: 'saeed' },
    ...overrides,
  };
}
function issue(overrides: Partial<GhIssue>): GhIssue {
  return {
    number: 1,
    title: 'an issue',
    html_url: 'https://example/i',
    state: 'open',
    created_at: iso(DAY_MS),
    updated_at: iso(DAY_MS),
    closed_at: null,
    comments: 0,
    labels: [],
    user: { login: 'saeed' },
    ...overrides,
  };
}
function run(overrides: Partial<GhWorkflowRun>): GhWorkflowRun {
  return {
    name: 'CI',
    path: '.github/workflows/ci-pipeline.yml',
    head_branch: 'main',
    status: 'completed',
    conclusion: 'success',
    created_at: iso(HOUR_MS),
    updated_at: iso(HOUR_MS),
    run_started_at: iso(HOUR_MS),
    html_url: 'https://example/run',
    event: 'push',
    id: 100,
    ...overrides,
  };
}

describe('median', () => {
  it('handles empty, odd, and even lengths', () => {
    expect(median([])).toBe(0);
    expect(median([3, 1, 2])).toBe(2);
    expect(median([4, 1, 2, 3])).toBe(2.5);
  });
});

describe('Delivery (DORA-lite)', () => {
  it('releasesPerWeek averages releases in the window', () => {
    const releases = [release(2 * DAY_MS), release(10 * DAY_MS), release(200 * DAY_MS)];
    expect(releasesPerWeek(releases, NOW, 8)).toBe(2 / 8);
  });

  it('medianLeadTimeHours measures merge → next release', () => {
    const releases = [release(4 * HOUR_MS), release(24 * HOUR_MS)];
    const pulls = [
      pull({ merged_at: iso(10 * HOUR_MS) }), // next release at 4h ago ⇒ 6h
      pull({ merged_at: iso(30 * HOUR_MS) }), // next release at 24h ago ⇒ 6h
    ];
    expect(medianLeadTimeHours(pulls, releases)).toBe(6);
  });

  it('commitTypeRatio counts fix:/revert: against typed commits', () => {
    const commits = [
      commit('fix: a'),
      commit('feat: b'),
      commit('revert: c'),
      commit('chore: d'),
      commit('not conventional at all'),
    ];
    const r = commitTypeRatio(commits);
    expect(r.typed).toBe(4);
    expect(r.fixish).toBe(2);
    expect(r.ratio).toBe(0.5);
  });

  it('commitTypeRatio yields a null ratio (not 0%) when nothing is typed', () => {
    expect(commitTypeRatio([])).toEqual({ fixish: 0, typed: 0, ratio: null });
    expect(commitTypeRatio([commit('no conventional prefix at all')]).ratio).toBeNull();
  });

  it('workflowHealth + latestGatingConclusion read gating runs on main', () => {
    const runs = [
      run({ conclusion: 'success', id: 3 }),
      run({ conclusion: 'success', id: 2 }),
      run({ conclusion: 'failure', id: 1 }),
      run({ path: '.github/workflows/security.yml', conclusion: 'failure', id: 0 }), // ignored
      run({ head_branch: 'feature', conclusion: 'failure', id: -1 }), // ignored
    ];
    const health = workflowHealth(runs, 'ci-pipeline.yml');
    expect(health.sampled).toBe(3);
    expect(health.successRate).toBeCloseTo(2 / 3);
    expect(latestGatingConclusion(runs, 'ci-pipeline.yml')).toBe('success');
  });

  it('workflowHealth is null (not 100%) on an empty sample', () => {
    const health = workflowHealth([], 'ci-pipeline.yml');
    expect(health.successRate).toBeNull();
    expect(health.sampled).toBe(0);
    expect(health.lastConclusion).toBeNull();
  });
});

describe('Work (PRs + issues)', () => {
  it('summarizeOpenPulls sorts by age and staleCount excludes drafts', () => {
    const pulls = [
      pull({ number: 1, created_at: iso(20 * DAY_MS) }),
      pull({ number: 2, created_at: iso(30 * DAY_MS), draft: true }),
      pull({ number: 3, created_at: iso(2 * DAY_MS) }),
      pull({ number: 4, merged_at: iso(DAY_MS) }), // merged ⇒ excluded
    ];
    const views = summarizeOpenPulls(pulls, NOW);
    expect(views.map((v) => v.number)).toEqual([2, 1, 3]);
    expect(staleCount(views, 14)).toBe(1); // #1 only (#2 is a draft)
  });

  it('realIssues drops PRs; criticalIssueCount + issuesByLabel + needsAttention read the rest', () => {
    const issues = [
      issue({ number: 1, labels: [{ name: 'critical' }, 'bug'] }),
      issue({ number: 2, labels: ['bug'], comments: 0, created_at: iso(40 * DAY_MS) }),
      issue({ number: 3, pull_request: { url: 'x' }, labels: [{ name: 'critical' }] }), // a PR
    ];
    expect(realIssues(issues)).toHaveLength(2);
    expect(criticalIssueCount(issues, ['critical'])).toBe(1);
    expect(issuesByLabel(issues)).toEqual([
      { label: 'bug', count: 2 },
      { label: 'critical', count: 1 },
    ]);
    const attention = needsAttention(issues, NOW, 14);
    expect(attention.map((a) => a.number)).toEqual([2]);
  });
});

describe('Quality (CHAOSS)', () => {
  it('computes the starter-health card, including the bus-factor gag', () => {
    const health = chaossHealth({
      issues: [issue({ comments: 2, created_at: iso(10 * HOUR_MS), updated_at: iso(5 * HOUR_MS) })],
      closedPulls: [
        pull({ merged_at: iso(DAY_MS), closed_at: iso(DAY_MS) }),
        pull({ merged_at: null, closed_at: iso(DAY_MS) }),
      ],
      releases: [release(2 * DAY_MS)],
      commits: [commit('fix: a', 'saeed')],
      nowMs: NOW,
    });
    expect(health.timeToFirstResponseHours).toBe(5);
    expect(health.changeRequestClosureRatio).toBe(0.5);
    expect(health.busFactor).toBe(1);
    expect(health.busFactorGag).toMatch(/do not hit him with a bus/);
  });

  it('reports the real distinct-author count when more than one contributes', () => {
    const health = chaossHealth({
      issues: [],
      closedPulls: [],
      releases: [],
      commits: [commit('feat: a', 'saeed'), commit('fix: b', 'other')],
      nowMs: NOW,
    });
    expect(health.busFactor).toBe(2);
    expect(health.busFactorGag).toBe('2');
  });

  it('returns null closure ratio + time-to-first-response on empty samples', () => {
    const health = chaossHealth({
      issues: [],
      closedPulls: [],
      releases: [],
      commits: [],
      nowMs: NOW,
    });
    expect(health.changeRequestClosureRatio).toBeNull();
    expect(health.timeToFirstResponseHours).toBeNull();
  });
});

describe('Community + benchmarks', () => {
  it('totalReleaseDownloads sums every asset', () => {
    expect(totalReleaseDownloads([release(DAY_MS, [3, 4]), release(2 * DAY_MS, [5])])).toBe(12);
  });

  it('commitsPerDay buckets commits into the trailing window', () => {
    const buckets = commitsPerDay([commit('a', 'x', 0), commit('b', 'x', DAY_MS)], NOW, 14);
    expect(buckets).toHaveLength(14);
    expect(buckets.at(-1)).toBe(1); // today
    expect(buckets.at(-2)).toBe(1); // yesterday
    expect(buckets.reduce((s, n) => s + n, 0)).toBe(2);
  });

  it('summarizeBenchmarks builds a mean trend + delta per suite', () => {
    const summaries = summarizeBenchmarks({
      'Export render': [
        { date: 1, benches: [{ name: 'a', value: 10, unit: 'ns/iter' }] },
        { date: 2, benches: [{ name: 'a', value: 20, unit: 'ns/iter' }] },
      ],
    });
    expect(summaries).toHaveLength(1);
    expect(summaries[0]?.values).toEqual([10, 20]);
    expect(summaries[0]?.latest).toBe(20);
    expect(summaries[0]?.deltaPct).toBe(100);
    expect(summaries[0]?.unit).toBe('ns/iter');
  });
});
