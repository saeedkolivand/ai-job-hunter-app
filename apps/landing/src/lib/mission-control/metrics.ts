import type { GhCommit, GhIssue, GhPull, GhRelease, GhWorkflowRun } from './types';

// Pure whole-repo metric computation over the GitHub payloads. Everything here
// is deterministic (state + `nowMs` in, numbers out) so it is exercised by unit
// tests with fixture payloads — no network, no clock.

export const HOUR_MS = 60 * 60 * 1000;
export const DAY_MS = 24 * HOUR_MS;
const WEEK_MS = 7 * DAY_MS;

export function median(values: readonly number[]): number {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  if (sorted.length % 2 === 1) return sorted[mid] ?? 0;
  return ((sorted[mid - 1] ?? 0) + (sorted[mid] ?? 0)) / 2;
}

// ── Delivery (DORA-lite) ─────────────────────────────────────────────────────
export function releasesPerWeek(releases: readonly GhRelease[], nowMs: number, weeks = 8): number {
  const windowStart = nowMs - weeks * WEEK_MS;
  const inWindow = releases.filter(
    (r) => r.published_at !== null && Date.parse(r.published_at) >= windowStart
  ).length;
  return inWindow / weeks;
}

// Median hours from a PR merge to the next release that shipped it.
export function medianLeadTimeHours(
  mergedPulls: readonly GhPull[],
  releases: readonly GhRelease[]
): number | null {
  const releaseTimes = releases
    .map((r) => (r.published_at !== null ? Date.parse(r.published_at) : Number.NaN))
    .filter((t) => !Number.isNaN(t))
    .sort((a, b) => a - b);

  const leads: number[] = [];
  for (const pr of mergedPulls) {
    if (pr.merged_at === null) continue;
    const merged = Date.parse(pr.merged_at);
    const next = releaseTimes.find((t) => t >= merged);
    if (next === undefined) continue;
    leads.push((next - merged) / HOUR_MS);
  }
  return leads.length > 0 ? median(leads) : null;
}

const CONVENTIONAL = /^(\w+)(\([^)]*\))?!?:/;

// Proxy for change-failure: the share of typed commits that are fix:/revert:.
export function commitTypeRatio(commits: readonly GhCommit[]): {
  fixish: number;
  typed: number;
  // null when there are no typed commits to divide by — the UI renders '—'
  // rather than a misleading 0%.
  ratio: number | null;
} {
  let typed = 0;
  let fixish = 0;
  for (const c of commits) {
    const subject = c.commit.message.split('\n', 1)[0] ?? '';
    const match = CONVENTIONAL.exec(subject);
    if (match) {
      typed += 1;
      const type = (match[1] ?? '').toLowerCase();
      if (type === 'fix' || type === 'revert') fixish += 1;
    } else if (/^revert/i.test(subject)) {
      typed += 1;
      fixish += 1;
    }
  }
  return { fixish, typed, ratio: typed > 0 ? fixish / typed : null };
}

function gatingRuns(runs: readonly GhWorkflowRun[], gatingFile: string): GhWorkflowRun[] {
  return runs.filter(
    (r) => r.path.endsWith(gatingFile) && r.head_branch === 'main' && r.status === 'completed'
  );
}

// skipped/neutral aren't pass/fail signals (GitHub treats them as success-
// equivalent for dependent checks), so they're excluded from the pass-rate
// denominator — otherwise routine concurrency-skips would drag the rate down.
function isScored(run: GhWorkflowRun): boolean {
  return run.conclusion !== 'skipped' && run.conclusion !== 'neutral';
}

export function workflowHealth(
  runs: readonly GhWorkflowRun[],
  gatingFile: string,
  sample = 20
): { successRate: number | null; sampled: number; lastConclusion: string | null } {
  const recent = gatingRuns(runs, gatingFile).slice(0, sample);
  const scored = recent.filter(isScored);
  const success = scored.filter((r) => r.conclusion === 'success').length;
  return {
    // null on an empty sample — a "100% · last 0 runs" reads as false confidence.
    successRate: scored.length > 0 ? success / scored.length : null,
    sampled: scored.length,
    lastConclusion: recent[0]?.conclusion ?? null,
  };
}

// The most recent DECISIVE gating run on main (drives the verdict) — looks past
// skipped/neutral runs so a concurrency-skip doesn't hide the real last result.
export function latestGatingConclusion(
  runs: readonly GhWorkflowRun[],
  gatingFile: string
): string | null {
  return gatingRuns(runs, gatingFile).find(isScored)?.conclusion ?? null;
}

// ── Work (PRs + issues) ──────────────────────────────────────────────────────
export interface PullView {
  number: number;
  title: string;
  url: string;
  ageDays: number;
  draft: boolean;
  awaitingReview: boolean;
}

export function summarizeOpenPulls(pulls: readonly GhPull[], nowMs: number): PullView[] {
  return pulls
    .filter((p) => p.merged_at === null && p.closed_at === null)
    .map((p) => ({
      number: p.number,
      title: p.title,
      url: p.html_url,
      ageDays: Math.floor((nowMs - Date.parse(p.created_at)) / DAY_MS),
      draft: p.draft,
      awaitingReview: p.requested_reviewers.length > 0,
    }))
    .sort((a, b) => b.ageDays - a.ageDays);
}

export function staleCount(views: readonly PullView[], staleDays: number): number {
  return views.filter((v) => !v.draft && v.ageDays > staleDays).length;
}

function labelName(label: { name: string } | string): string {
  return typeof label === 'string' ? label : label.name;
}

// The /issues endpoint mixes in PRs (they carry `pull_request`) — drop them.
export function realIssues(issues: readonly GhIssue[]): GhIssue[] {
  return issues.filter((i) => i.pull_request === undefined);
}

export function criticalIssueCount(
  issues: readonly GhIssue[],
  criticalLabels: readonly string[]
): number {
  const set = new Set(criticalLabels.map((s) => s.toLowerCase()));
  return realIssues(issues).filter((i) => i.labels.some((l) => set.has(labelName(l).toLowerCase())))
    .length;
}

export function issuesByLabel(issues: readonly GhIssue[]): { label: string; count: number }[] {
  const counts = new Map<string, number>();
  for (const issue of realIssues(issues)) {
    for (const label of issue.labels) {
      const name = labelName(label);
      counts.set(name, (counts.get(name) ?? 0) + 1);
    }
  }
  return [...counts.entries()]
    .map(([label, count]) => ({ label, count }))
    .sort((a, b) => b.count - a.count);
}

export interface IssueView {
  number: number;
  title: string;
  url: string;
  ageDays: number;
}

export function needsAttention(
  issues: readonly GhIssue[],
  nowMs: number,
  staleDays: number
): IssueView[] {
  return realIssues(issues)
    .filter((i) => i.comments === 0 && (nowMs - Date.parse(i.created_at)) / DAY_MS > staleDays)
    .map((i) => ({
      number: i.number,
      title: i.title,
      url: i.html_url,
      ageDays: Math.floor((nowMs - Date.parse(i.created_at)) / DAY_MS),
    }))
    .sort((a, b) => b.ageDays - a.ageDays);
}

// ── Quality (CHAOSS starter health) ──────────────────────────────────────────
export interface ChaossHealth {
  timeToFirstResponseHours: number | null;
  // null when no PRs closed in the window — the UI renders '—' not a fake 100%.
  changeRequestClosureRatio: number | null;
  releasesPerWeek: number;
  busFactor: number;
  busFactorGag: string;
}

export function chaossHealth(input: {
  issues: readonly GhIssue[];
  closedPulls: readonly GhPull[];
  releases: readonly GhRelease[];
  commits: readonly GhCommit[];
  nowMs: number;
}): ChaossHealth {
  // Time-to-first-response proxy: (updated − created) for issues that got a reply.
  const responded = realIssues(input.issues).filter((i) => i.comments > 0);
  const ttfr = responded.map(
    (i) => (Date.parse(i.updated_at) - Date.parse(i.created_at)) / HOUR_MS
  );

  const closed = input.closedPulls.filter((p) => p.closed_at !== null);
  const merged = closed.filter((p) => p.merged_at !== null).length;
  const changeRequestClosureRatio = closed.length > 0 ? merged / closed.length : null;

  const authors = new Set<string>();
  for (const c of input.commits) {
    const login = c.author?.login;
    if (login) authors.add(login);
  }
  const busFactor = Math.max(1, authors.size);

  return {
    timeToFirstResponseHours: ttfr.length > 0 ? median(ttfr) : null,
    changeRequestClosureRatio,
    releasesPerWeek: releasesPerWeek(input.releases, input.nowMs),
    busFactor,
    busFactorGag: busFactor <= 1 ? '1 — please do not hit him with a bus' : String(busFactor),
  };
}

// ── Community ────────────────────────────────────────────────────────────────
export function totalReleaseDownloads(releases: readonly GhRelease[]): number {
  let total = 0;
  for (const release of releases) {
    for (const asset of release.assets) total += asset.download_count;
  }
  return total;
}

export function commitsPerDay(commits: readonly GhCommit[], nowMs: number, days = 14): number[] {
  const buckets = Array.from({ length: days }, () => 0);
  for (const c of commits) {
    const date = c.commit.author?.date;
    if (!date) continue;
    const ageDays = Math.floor((nowMs - Date.parse(date)) / DAY_MS);
    if (ageDays >= 0 && ageDays < days) {
      const index = days - 1 - ageDays;
      buckets[index] = (buckets[index] ?? 0) + 1;
    }
  }
  return buckets;
}

// ── Benchmarks (same-origin /benchmarks/data.js) ─────────────────────────────
export interface BenchRun {
  date: number;
  benches: { name: string; value: number; unit: string }[];
}
export interface BenchSummary {
  name: string;
  values: number[];
  latest: number;
  unit: string;
  deltaPct: number | null;
}

export function summarizeBenchmarks(
  entries: Record<string, BenchRun[]>,
  take = 16
): BenchSummary[] {
  const out: BenchSummary[] = [];
  for (const [suite, runs] of Object.entries(entries)) {
    const ordered = [...runs].sort((a, b) => a.date - b.date);
    const series = ordered.map((run) => {
      const values = run.benches.map((b) => b.value);
      return values.length > 0 ? values.reduce((sum, v) => sum + v, 0) / values.length : 0;
    });
    const values = series.slice(-take);
    const latest = values[values.length - 1] ?? 0;
    const prev = values[values.length - 2];
    const unit = ordered[ordered.length - 1]?.benches[0]?.unit ?? '';
    out.push({
      name: suite,
      values,
      latest,
      unit,
      deltaPct: prev !== undefined && prev !== 0 ? ((latest - prev) / prev) * 100 : null,
    });
  }
  return out;
}
