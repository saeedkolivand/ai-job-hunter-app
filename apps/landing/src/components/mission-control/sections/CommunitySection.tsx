import { fmtInt, type Model } from '@/lib/mission-control/model';

import { Section } from '../Section';
import { Sparkline } from '../Sparkline';
import { Stat } from '../Stat';

export function CommunitySection({ community }: { community: Model['community'] }) {
  return (
    <Section label="Community" eyebrow="who is watching" title="Community">
      <div className="mc-grid">
        <Stat num={fmtInt(community.repo?.stargazers_count ?? 0)} label="Stars" />
        <Stat num={fmtInt(community.repo?.forks_count ?? 0)} label="Forks" />
        <Stat num={fmtInt(community.repo?.subscribers_count ?? 0)} label="Watchers" />
        <Stat
          num={fmtInt(community.downloads)}
          label="Release asset downloads"
          sub="all releases, all platforms"
        />
      </div>
      <div className="mc-card" style={{ marginTop: '14px' }}>
        <span className="mc-stat__label">Commit activity · last 21 days</span>
        <Sparkline
          values={community.commitActivity}
          label="Commits per day over the last 21 days"
          stroke="var(--doc-series-2)"
        />
      </div>
    </Section>
  );
}
