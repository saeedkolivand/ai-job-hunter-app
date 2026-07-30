import type { BenchSummary } from '@/lib/mission-control/metrics';
import { fmtInt, type Model, pctOrDash, round1 } from '@/lib/mission-control/model';

import { Section } from '../Section';
import { Sparkline } from '../Sparkline';
import { Stat } from '../Stat';

// Sparkline series colors — hoisted module constant, was recreated every render.
const SERIES_COLORS = [
  'var(--doc-series-1)',
  'var(--doc-series-2)',
  'var(--doc-series-3)',
  'var(--doc-series-4)',
];

export function QualitySection({
  quality,
  benchSummaries,
}: {
  quality: Model['quality'];
  benchSummaries: BenchSummary[];
}) {
  return (
    <Section label="Quality" eyebrow="is it actually healthy" title="Quality">
      <div className="mc-grid">
        <Stat
          num={
            quality.chaoss.timeToFirstResponseHours === null
              ? '—'
              : round1(quality.chaoss.timeToFirstResponseHours)
          }
          unit={quality.chaoss.timeToFirstResponseHours === null ? undefined : 'h'}
          label="Time to first response"
          sub="median (proxy: reply latency on commented issues)"
        />
        <Stat
          num={pctOrDash(quality.chaoss.changeRequestClosureRatio)}
          label="Change-request closure"
          sub="merged ÷ all recently closed PRs"
        />
        <Stat
          num={round1(quality.chaoss.releasesPerWeek)}
          unit="/wk"
          label="Release frequency"
          sub="CHAOSS starter health"
        />
        <Stat
          num={quality.chaoss.busFactorGag}
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
                stroke={SERIES_COLORS[index % SERIES_COLORS.length]}
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
    </Section>
  );
}
