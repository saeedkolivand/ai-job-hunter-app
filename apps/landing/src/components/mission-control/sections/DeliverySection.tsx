import { type Model, pctOrDash, round1 } from '@/lib/mission-control/model';

import { ExternalLink } from '../ExternalLink';
import { Section } from '../Section';
import { Stat } from '../Stat';

export function DeliverySection({ delivery }: { delivery: Model['delivery'] }) {
  return (
    <Section label="Delivery" eyebrow="how fast it ships" title="Delivery">
      <div className="mc-grid">
        <Stat
          num={round1(delivery.perWeek)}
          unit="/wk"
          label="Releases per week"
          sub="8-week trailing average"
        />
        <Stat
          num={delivery.leadHours === null ? '—' : round1(delivery.leadHours)}
          unit={delivery.leadHours === null ? undefined : 'h'}
          label="Merge → release lead time"
          sub="median, DORA-lite"
        />
        <Stat
          num={pctOrDash(delivery.commitRatio.ratio)}
          label="fix: / revert: commits"
          sub={`${delivery.commitRatio.fixish} of ${delivery.commitRatio.typed} typed commits (change-failure proxy)`}
        />
        <Stat
          num={pctOrDash(delivery.health.successRate)}
          label="CI gating pass rate"
          sub={`last ${delivery.health.sampled} runs on main · latest: ${delivery.health.lastConclusion ?? 'unknown'}`}
        />
      </div>
      {delivery.recentReleases.length > 0 ? (
        <ul className="mc-list" style={{ marginTop: '14px' }}>
          {delivery.recentReleases.map((release) => (
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
    </Section>
  );
}
