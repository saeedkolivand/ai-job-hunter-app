import { fmtInt, type Model } from '@/lib/mission-control/model';
import {
  actionById,
  type WriteAction,
  type WriteActionContext,
} from '@/lib/mission-control/write-actions';

import { ExternalLink } from '../ExternalLink';
import { Section } from '../Section';
import { Stat } from '../Stat';

export function WorkSection({
  work,
  staleDays,
  signedIn,
  runAction,
}: {
  work: Model['work'];
  staleDays: number;
  signedIn: boolean;
  runAction: (action: WriteAction, ctx: WriteActionContext, danger?: boolean) => void;
}) {
  return (
    <Section label="Work" eyebrow="what needs a human" title="Work">
      <div className="mc-grid">
        <Stat
          num={fmtInt(work.totalOpenPulls)}
          label="Open pull requests"
          sub={`${work.stale} gathering dust > ${staleDays}d`}
        />
        <Stat
          num={fmtInt(work.critical)}
          label="Critical issues open"
          sub={work.critical === 0 ? 'nothing on fire' : 'triage first'}
        />
        <Stat
          num={fmtInt(work.attention.length)}
          label="Issues needing attention"
          sub="no reply + stale"
        />
      </div>

      {work.openPullViews.length > 0 ? (
        <ul className="mc-list" style={{ marginTop: '14px' }}>
          {work.openPullViews.map((pr) => (
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
              {!pr.draft && pr.ageDays > staleDays ? (
                <span className="mc-badge is-stale">stale</span>
              ) : null}
            </li>
          ))}
        </ul>
      ) : (
        <p className="mc-empty">no open pull requests</p>
      )}

      {work.attention.length > 0 ? (
        <ul className="mc-list" style={{ marginTop: '14px' }}>
          {work.attention.map((issue) => (
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
    </Section>
  );
}
