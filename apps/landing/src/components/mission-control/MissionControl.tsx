'use client';

import { type ReactNode, useCallback, useEffect, useMemo, useState } from 'react';

import { MC_CONFIG } from '@/lib/mission-control/config';
import {
  fetchSnapshotStamp,
  ghGet,
  liveOrSnapshot,
  snapshotFreshnessLine,
} from '@/lib/mission-control/github';
import {
  type BenchRun,
  chaossHealth,
  commitsPerDay,
  commitTypeRatio,
  criticalIssueCount,
  DAY_MS,
  issuesByLabel,
  latestGatingConclusion,
  medianLeadTimeHours,
  needsAttention,
  releasesPerWeek,
  staleCount,
  summarizeBenchmarks,
  summarizeOpenPulls,
  totalReleaseDownloads,
  workflowHealth,
} from '@/lib/mission-control/metrics';
import { clearToken, readToken, saveToken } from '@/lib/mission-control/pat';
import type {
  GhCommit,
  GhIssue,
  GhPull,
  GhRelease,
  GhRepo,
  GhWorkflowRun,
} from '@/lib/mission-control/types';
import { computeVerdict, isRedConclusion } from '@/lib/mission-control/verdict';
import {
  performWriteAction,
  WRITE_ACTIONS,
  type WriteAction,
  type WriteActionContext,
} from '@/lib/mission-control/write-actions';

import { ConfirmDialog } from './ConfirmDialog';
import { SignInPanel } from './SignInPanel';
import { Sparkline } from './Sparkline';

interface RepoData {
  repo: GhRepo | null;
  releases: GhRelease[];
  commits: GhCommit[];
  openPulls: GhPull[];
  closedPulls: GhPull[];
  issues: GhIssue[];
  runs: GhWorkflowRun[];
}

interface BenchmarkGlobal {
  BENCHMARK_DATA?: { entries?: Record<string, BenchRun[]> };
}

const fmtInt = (n: number): string => new Intl.NumberFormat().format(Math.round(n));
const round1 = (n: number): string => (Math.round(n * 10) / 10).toFixed(1);
// null (empty sample) renders as an em dash, never a misleading 0% / 100%.
const pctOrDash = (n: number | null): string => (n === null ? '—' : `${Math.round(n * 100)}%`);

function actionById(id: string): WriteAction {
  const action = WRITE_ACTIONS.find((a) => a.id === id);
  if (!action) throw new Error(`unknown write action ${id}`);
  return action;
}

async function safe<T>(promise: Promise<T>, fallback: T): Promise<T> {
  try {
    return await promise;
  } catch {
    return fallback;
  }
}

async function loadAll(token: string): Promise<RepoData> {
  const src = MC_CONFIG.dataSource;
  const get = <T,>(key: string, path: string) =>
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

function buildModel(data: RepoData) {
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
      labels: issuesByLabel(data.issues).slice(0, 8),
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

type Model = ReturnType<typeof buildModel>;

// ── small presentational helpers ─────────────────────────────────────────────
function Stat({
  num,
  unit,
  label,
  sub,
}: {
  num: string;
  unit?: string;
  label: string;
  sub?: string;
}) {
  return (
    <div className="mc-card">
      <p>
        <span className="mc-stat__num">{num}</span>
        {unit ? <span className="mc-stat__unit">{unit}</span> : null}
      </p>
      <p className="mc-stat__label">{label}</p>
      {sub ? <p className="mc-stat__sub">{sub}</p> : null}
    </div>
  );
}

function ExternalLink({ href, children }: { href: string; children: ReactNode }) {
  return (
    <a href={href} target="_blank" rel="noopener noreferrer">
      {children}
    </a>
  );
}

// First-load placeholders so the page never shows a big blank gap while the
// GitHub API resolves. Decorative (aria-hidden); the busy state is announced once.
function SkeletonGrid({ n = 4 }: { n?: number }) {
  return (
    <div className="mc-grid" aria-hidden="true">
      {Array.from({ length: n }, (_, i) => (
        <div className="mc-card mc-skeleton" key={i}>
          <div className="mc-skel" style={{ height: '34px', width: '58%', marginBottom: '10px' }} />
          <div className="mc-skel" style={{ height: '10px', width: '80%' }} />
        </div>
      ))}
    </div>
  );
}

function MissionControlSkeleton() {
  return (
    <div aria-busy="true" aria-label="Loading whole-repo state">
      <div className="mc-verdict mc-skeleton" aria-hidden="true">
        <div className="mc-skel" style={{ height: '16px', width: '120px', marginBottom: '10px' }} />
        <div className="mc-skel" style={{ height: '30px', width: '70%' }} />
      </div>
      <div className="mc-section" aria-hidden="true">
        <div className="mc-skel" style={{ height: '26px', width: '180px', marginBottom: '16px' }} />
        <SkeletonGrid />
      </div>
      <div className="mc-section" aria-hidden="true">
        <div className="mc-skel" style={{ height: '26px', width: '150px', marginBottom: '16px' }} />
        <SkeletonGrid n={3} />
      </div>
    </div>
  );
}

export function MissionControl() {
  const [token, setToken] = useState('');
  const [data, setData] = useState<RepoData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const [freshness, setFreshness] = useState<string | null>(null);
  const [bench, setBench] = useState<Record<string, BenchRun[]> | null>(null);
  const [actionStatus, setActionStatus] = useState<string | null>(null);
  const [pending, setPending] = useState<{
    message: string;
    danger: boolean;
    resolve: (ok: boolean) => void;
  } | null>(null);

  // Clickjacking guard. A <meta> CSP's `frame-ancestors` is INERT (CSP3 ignores it
  // in meta), so the page is framable on GitHub Pages — bust out of any frame so
  // signed-in write actions can never be driven from inside someone else's page.
  useEffect(() => {
    try {
      if (window.self !== window.top && window.top) {
        window.top.location.href = window.self.location.href;
      }
    } catch {
      // Cross-origin top may block the navigation; nothing more a static page can do.
    }
  }, []);

  // Read the stored token once on mount (SSR-safe: never touch storage at module scope).
  useEffect(() => {
    setToken(readToken());
  }, []);

  // Data load — syncing with an external system (GitHub), so an effect is correct.
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    loadAll(token)
      .then((result) => {
        if (!cancelled) setData(result);
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(err instanceof Error ? err.message : 'Failed to load repo data.');
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    // Honest-UI: stamp the snapshot's age when one is present. Renders nothing in
    // live mode or before the first nightly snapshot exists (meta.json 404s).
    void fetchSnapshotStamp(MC_CONFIG.dataSource).then((stamp) => {
      if (!cancelled) setFreshness(stamp);
    });
    return () => {
      cancelled = true;
    };
  }, [token, reloadKey]);

  // Same-origin benchmark series: inject /benchmarks/data.js once, read the global.
  useEffect(() => {
    if (document.querySelector('script[data-mc-bench]')) {
      const existing = (window as unknown as BenchmarkGlobal).BENCHMARK_DATA?.entries;
      if (existing) setBench(existing);
      return;
    }
    const script = document.createElement('script');
    script.src = MC_CONFIG.benchmarksSrc;
    script.dataset.mcBench = 'true';
    script.onload = () => {
      const entries = (window as unknown as BenchmarkGlobal).BENCHMARK_DATA?.entries;
      if (entries) setBench(entries);
    };
    document.body.appendChild(script);
  }, []);

  const confirm = useCallback(
    (message: string, danger = false) =>
      new Promise<boolean>((resolve) => setPending({ message, danger, resolve })),
    []
  );

  const runAction = useCallback(
    async (action: WriteAction, ctx: WriteActionContext, danger = false) => {
      setActionStatus(null);
      const outcome = await performWriteAction(action, ctx, {
        token,
        confirm: (message) => confirm(message, danger),
      });
      if (outcome.status === 'cancelled') return;
      if (outcome.result.ok) {
        setActionStatus(`${action.label}: done (HTTP ${outcome.result.status}). Refreshing…`);
        setReloadKey((k) => k + 1);
      } else {
        setActionStatus(`${action.label}: GitHub returned HTTP ${outcome.result.status}.`);
      }
    },
    [token, confirm]
  );

  const model = useMemo<Model | null>(() => (data ? buildModel(data) : null), [data]);
  const freshnessLine = useMemo(
    () => (freshness ? snapshotFreshnessLine(freshness, Date.now()) : null),
    [freshness]
  );
  const benchSummaries = useMemo(() => (bench ? summarizeBenchmarks(bench) : []), [bench]);
  const signedIn = token.length > 0;

  const seriesColors = [
    'var(--doc-series-1)',
    'var(--doc-series-2)',
    'var(--doc-series-3)',
    'var(--doc-series-4)',
  ];

  return (
    <div className="mc">
      <div className="mc-toolbar">
        <button
          type="button"
          className="mc-btn"
          onClick={() => setReloadKey((k) => k + 1)}
          disabled={loading}
          aria-busy={loading}
        >
          {loading ? '↻ Refreshing…' : '↻ Refresh'}
        </button>
        <span className="mc-grow">
          {loading
            ? 'loading whole-repo state…'
            : signedIn
              ? 'signed in · 5,000/h'
              : 'anonymous · 60/h'}
        </span>
        <ExternalLink href={`${MC_CONFIG.htmlBase}/actions`}>Actions ↗</ExternalLink>
      </div>

      <SignInPanel
        signedIn={signedIn}
        onSignIn={(value) => {
          saveToken(value);
          setToken(readToken());
        }}
        onSignOut={() => {
          clearToken();
          setToken('');
        }}
      />

      {error ? (
        <p className="mc-status is-error" role="alert">
          {error}
        </p>
      ) : null}
      {actionStatus ? (
        <p className="mc-status" role="status">
          {actionStatus}
        </p>
      ) : null}

      {model ? (
        <>
          {/* ── verdict hero ── */}
          <section
            className={`mc-verdict is-${model.verdict.tone}`}
            aria-label="Repository verdict"
          >
            <p className="mc-verdict__eyebrow">the verdict</p>
            <p className="mc-verdict__line">{model.verdict.line}</p>
            <p className="mc-verdict__sub">{model.verdict.sub}</p>
          </section>

          {/* ── Delivery ── */}
          <section className="mc-section" aria-label="Delivery">
            <p className="mc-section__eyebrow">how fast it ships</p>
            <h2 className="mc-section__title">Delivery</h2>
            <div className="mc-grid">
              <Stat
                num={round1(model.delivery.perWeek)}
                unit="/wk"
                label="Releases per week"
                sub="8-week trailing average"
              />
              <Stat
                num={model.delivery.leadHours === null ? '—' : round1(model.delivery.leadHours)}
                unit={model.delivery.leadHours === null ? undefined : 'h'}
                label="Merge → release lead time"
                sub="median, DORA-lite"
              />
              <Stat
                num={pctOrDash(model.delivery.commitRatio.ratio)}
                label="fix: / revert: commits"
                sub={`${model.delivery.commitRatio.fixish} of ${model.delivery.commitRatio.typed} typed commits (change-failure proxy)`}
              />
              <Stat
                num={pctOrDash(model.delivery.health.successRate)}
                label="CI gating pass rate"
                sub={`last ${model.delivery.health.sampled} runs on main · latest: ${model.delivery.health.lastConclusion ?? 'unknown'}`}
              />
            </div>
            {model.delivery.recentReleases.length > 0 ? (
              <ul className="mc-list" style={{ marginTop: '14px' }}>
                {model.delivery.recentReleases.map((release) => (
                  <li key={release.tag_name} className="mc-row">
                    <span className="mc-row__title">
                      <ExternalLink href={release.html_url}>
                        {release.name ?? release.tag_name}
                      </ExternalLink>
                    </span>
                    <span className="mc-row__meta">
                      {release.published_at
                        ? new Date(release.published_at).toLocaleDateString()
                        : 'draft'}
                    </span>
                  </li>
                ))}
              </ul>
            ) : null}
          </section>

          {/* ── Work ── */}
          <section className="mc-section" aria-label="Work">
            <p className="mc-section__eyebrow">what needs a human</p>
            <h2 className="mc-section__title">Work</h2>
            <div className="mc-grid">
              <Stat
                num={fmtInt(model.work.totalOpenPulls)}
                label="Open pull requests"
                sub={`${model.work.stale} gathering dust > ${MC_CONFIG.staleDays}d`}
              />
              <Stat
                num={fmtInt(model.work.critical)}
                label="Critical issues open"
                sub={model.work.critical === 0 ? 'nothing on fire' : 'triage first'}
              />
              <Stat
                num={fmtInt(model.work.attention.length)}
                label="Issues needing attention"
                sub="no reply + stale"
              />
            </div>

            {model.work.openPullViews.length > 0 ? (
              <ul className="mc-list" style={{ marginTop: '14px' }}>
                {model.work.openPullViews.map((pr) => (
                  <li key={pr.number} className="mc-row">
                    <span className="mc-row__title">
                      <ExternalLink href={pr.url}>
                        #{pr.number} {pr.title}
                      </ExternalLink>
                    </span>
                    <span className="mc-row__meta">{pr.ageDays}d old</span>
                    {pr.draft ? <span className="mc-badge is-draft">draft</span> : null}
                    {pr.awaitingReview ? (
                      <span className="mc-badge is-review">review requested</span>
                    ) : null}
                    {!pr.draft && pr.ageDays > MC_CONFIG.staleDays ? (
                      <span className="mc-badge is-stale">stale</span>
                    ) : null}
                  </li>
                ))}
              </ul>
            ) : (
              <p className="mc-empty">no open pull requests</p>
            )}

            {model.work.attention.length > 0 ? (
              <ul className="mc-list" style={{ marginTop: '14px' }}>
                {model.work.attention.map((issue) => (
                  <li key={issue.number} className="mc-row">
                    <span className="mc-row__title">
                      <ExternalLink href={issue.url}>
                        #{issue.number} {issue.title}
                      </ExternalLink>
                    </span>
                    <span className="mc-row__meta">{issue.ageDays}d, no reply</span>
                    {signedIn ? (
                      <span className="mc-row__actions">
                        <button
                          type="button"
                          className="mc-btn"
                          onClick={() =>
                            void runAction(actionById('close-issue'), { issueNumber: issue.number })
                          }
                        >
                          Close
                        </button>
                        <button
                          type="button"
                          className="mc-btn"
                          onClick={() =>
                            void runAction(actionById('label-issue'), {
                              issueNumber: issue.number,
                              label: 'needs-triage',
                            })
                          }
                        >
                          +triage
                        </button>
                      </span>
                    ) : null}
                  </li>
                ))}
              </ul>
            ) : null}
          </section>

          {/* ── Quality ── */}
          <section className="mc-section" aria-label="Quality">
            <p className="mc-section__eyebrow">is it actually healthy</p>
            <h2 className="mc-section__title">Quality</h2>
            <div className="mc-grid">
              <Stat
                num={
                  model.quality.chaoss.timeToFirstResponseHours === null
                    ? '—'
                    : round1(model.quality.chaoss.timeToFirstResponseHours)
                }
                unit={model.quality.chaoss.timeToFirstResponseHours === null ? undefined : 'h'}
                label="Time to first response"
                sub="median (proxy: reply latency on commented issues)"
              />
              <Stat
                num={pctOrDash(model.quality.chaoss.changeRequestClosureRatio)}
                label="Change-request closure"
                sub="merged ÷ all recently closed PRs"
              />
              <Stat
                num={round1(model.quality.chaoss.releasesPerWeek)}
                unit="/wk"
                label="Release frequency"
                sub="CHAOSS starter health"
              />
              <Stat
                num={model.quality.chaoss.busFactorGag}
                label="Bus factor"
                sub="distinct recent commit authors"
              />
            </div>

            {benchSummaries.length > 0 ? (
              <div className="mc-grid" style={{ marginTop: '14px' }}>
                {benchSummaries.map((suite, index) => (
                  <div className="mc-card" key={suite.name}>
                    <div className="mc-card__head">
                      <span className="mc-stat__label">{suite.name}</span>
                      {suite.deltaPct !== null ? (
                        <span className="mc-row__meta">
                          {suite.deltaPct >= 0 ? '+' : ''}
                          {round1(suite.deltaPct)}%
                        </span>
                      ) : null}
                    </div>
                    <Sparkline
                      values={suite.values}
                      label={`${suite.name} benchmark trend`}
                      stroke={seriesColors[index % seriesColors.length]}
                    />
                    <p className="mc-stat__sub">
                      latest {fmtInt(suite.latest)} {suite.unit}
                    </p>
                  </div>
                ))}
              </div>
            ) : (
              <p className="mc-empty" style={{ marginTop: '14px' }}>
                benchmark series not loaded
              </p>
            )}
          </section>

          {/* ── Community ── */}
          <section className="mc-section" aria-label="Community">
            <p className="mc-section__eyebrow">who is watching</p>
            <h2 className="mc-section__title">Community</h2>
            <div className="mc-grid">
              <Stat num={fmtInt(model.community.repo?.stargazers_count ?? 0)} label="Stars" />
              <Stat num={fmtInt(model.community.repo?.forks_count ?? 0)} label="Forks" />
              <Stat num={fmtInt(model.community.repo?.subscribers_count ?? 0)} label="Watchers" />
              <Stat
                num={fmtInt(model.community.downloads)}
                label="Release asset downloads"
                sub="all releases, all platforms"
              />
            </div>
            <div className="mc-card" style={{ marginTop: '14px' }}>
              <span className="mc-stat__label">Commit activity · last 21 days</span>
              <Sparkline
                values={model.community.commitActivity}
                label="Commits per day over the last 21 days"
                stroke="var(--doc-series-2)"
              />
            </div>
          </section>

          {/* ── Maintainer actions (signed-in, safe tier) ── */}
          {signedIn ? (
            <section className="mc-section" aria-label="Maintainer actions">
              <p className="mc-section__eyebrow">every one asks first, none of them merge</p>
              <h2 className="mc-section__title">Maintainer actions</h2>
              <div className="mc-maintainer">
                <button
                  type="button"
                  className="mc-btn is-primary"
                  onClick={() => void runAction(actionById('dispatch-release'), {}, true)}
                >
                  Dispatch release workflow
                </button>
                <button
                  type="button"
                  className="mc-btn is-danger"
                  onClick={() => void runAction(actionById('dispatch-pages'), {}, true)}
                >
                  Dispatch pages deploy
                </button>
                {model.work.failedGatingRun ? (
                  <button
                    type="button"
                    className="mc-btn is-danger"
                    onClick={() =>
                      void runAction(actionById('rerun-failed'), {
                        runId: model.work.failedGatingRun?.id,
                      })
                    }
                  >
                    Re-run failed CI (#{model.work.failedGatingRun.id})
                  </button>
                ) : null}
              </div>
            </section>
          ) : null}
        </>
      ) : loading ? (
        <MissionControlSkeleton />
      ) : error ? null : (
        // On a hard failure the role="alert" above is the single message (U10).
        <p className="mc-empty">Could not load the dashboard. Try Refresh.</p>
      )}

      {freshnessLine ? <p className="mc-status">{freshnessLine}</p> : null}

      <ConfirmDialog
        open={pending !== null}
        title="Confirm write action"
        message={pending?.message ?? ''}
        confirmLabel="Yes, do it"
        danger={pending?.danger ?? false}
        onConfirm={() => {
          pending?.resolve(true);
          setPending(null);
        }}
        onCancel={() => {
          pending?.resolve(false);
          setPending(null);
        }}
      />
    </div>
  );
}
