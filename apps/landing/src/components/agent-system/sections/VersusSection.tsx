import { DIVIDER_VERSUS } from '@/data/agent-fleet';

import { Divider } from '../Divider';

export function VersusSection() {
  return (
    <>
      <p className="scrawl reveal">is it worth it?</p>
      <h2 className="section reveal">With vs Without the Fleet</h2>
      <p className="section-sub reveal">
        same job. honestly: agents cost a token premium for higher reliability + parallel speed.
      </p>

      <div className="versus">
        <div className="vcol without reveal">
          <h3>One agent, no fleet</h3>
          <div className="dim">
            <span className="k">Wall-clock</span>
            <span className="v">
              Serial. One context does intake, code, review, tests, docs — back to back.
            </span>
          </div>
          <div className="dim">
            <span className="k">Tokens</span>
            <span className="v">
              Cold re-exploration every spawn: <b>≈70–122k tokens/spawn</b>, <b>≈120 tool-uses</b>{' '}
              to rebuild context it never kept.
            </span>
          </div>
          <div className="dim">
            <span className="k">Bugs caught before release</span>
            <span className="v">
              The writer reviews its own work — the missed error is the one it didn&rsquo;t see the
              first time. No release gate.
            </span>
          </div>
        </div>
        <div className="vcol with reveal">
          <h3>
            The agent fleet <span className="ink-badge">↑ this one</span>
          </h3>
          <div className="dim">
            <span className="k">Wall-clock</span>
            <span className="v">
              Authors run in parallel on disjoint files; critics audit alongside. Faster on
              cross-layer work.
            </span>
          </div>
          <div className="dim">
            <span className="k">Tokens</span>
            <span className="v">
              Pre-harvested handoff instead of cold start: <b>≈44 tool-uses</b> vs ≈120. Multi-agent
              adds <b>≈+58% token overhead</b> — kept small by the handoff.
            </span>
          </div>
          <div className="dim">
            <span className="k">Bugs caught before release</span>
            <span className="v">
              An independent writer/critic split catches <b>≈60–80%</b> of otherwise-missed errors,
              plus the Stop review-gate before merge.
            </span>
          </div>
        </div>
      </div>
      <p className="illustrative reveal">
        <b>ILLUSTRATIVE FIGURES.</b> The numbers above (cold-start ≈70–122k tokens/spawn; ≈44 vs
        ≈120 tool-uses pre-harvested vs cold; writer/critic catching ≈60–80% of missed errors;
        multi-agent ≈+58% token overhead) are illustrative, not measured benchmarks of this repo.
        The honest trade: agents cost a token premium for higher reliability and parallel speed; the
        pre-harvested handoff is what keeps that premium small.
      </p>

      <Divider d={DIVIDER_VERSUS} />
    </>
  );
}
