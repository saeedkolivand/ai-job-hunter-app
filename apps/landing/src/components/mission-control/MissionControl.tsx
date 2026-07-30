'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';

import { MC_CONFIG } from '@/lib/mission-control/config';
import { fetchSnapshotStamp, snapshotFreshnessLine } from '@/lib/mission-control/github';
import { type BenchRun, summarizeBenchmarks } from '@/lib/mission-control/metrics';
import {
  type BenchmarkGlobal,
  buildModel,
  loadAll,
  type Model,
  type RepoData,
} from '@/lib/mission-control/model';
import { clearToken, readToken, saveToken } from '@/lib/mission-control/pat';
import {
  performWriteAction,
  type WriteAction,
  type WriteActionContext,
} from '@/lib/mission-control/write-actions';

import { ConfirmDialog } from './ConfirmDialog';
import { ExternalLink } from './ExternalLink';
import { MissionControlSkeleton } from './MissionControlSkeleton';
import { CommunitySection } from './sections/CommunitySection';
import { DeliverySection } from './sections/DeliverySection';
import { MaintainerActions } from './sections/MaintainerActions';
import { QualitySection } from './sections/QualitySection';
import { WorkSection } from './sections/WorkSection';
import { SignInPanel } from './SignInPanel';

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

          <DeliverySection delivery={model.delivery} />
          <WorkSection
            work={model.work}
            staleDays={MC_CONFIG.staleDays}
            signedIn={signedIn}
            runAction={runAction}
          />
          <QualitySection quality={model.quality} benchSummaries={benchSummaries} />
          <CommunitySection community={model.community} />
          {signedIn ? (
            <MaintainerActions failedGatingRun={model.work.failedGatingRun} runAction={runAction} />
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
