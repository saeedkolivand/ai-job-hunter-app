import { AGENT_COUNT, DIVIDER_HERO, HERO_SCENE } from '@/data/agent-fleet';

import { Divider } from '../Divider';

export function HeroSection() {
  return (
    <>
      <p className="scrawl reveal">the .claude/ agent system</p>
      <h1 className="reveal">The Agent Fleet</h1>
      <p className="tag reveal">
        a swarm of tiny specialists builds this repo, and a second swarm{' '}
        <em>tears their work apart.</em>
      </p>
      <p className="count reveal">
        <span className="num" data-count={AGENT_COUNT}>
          {AGENT_COUNT}
        </span>{' '}
        <b>agents</b> · paired author + critic per domain · nobody approves their own work.
      </p>

      <p className="note reveal">
        <b>This page is about how this repo gets built</b>, not what the app does. The fleet below
        is a development-time tool — Claude Code agents that write and review the code you&rsquo;re
        reading. The product&rsquo;s own AI pipeline (evidence-grounded generation, deterministic
        validators, prompt-injection fencing, an offline eval harness) is a separate system — see{' '}
        <a href="/how-it-works">how it works</a>.
      </p>

      <div className="hero-scene reveal draw" aria-hidden="true">
        <svg viewBox={HERO_SCENE.viewBox}>
          <g className="glow">
            <rect
              className="ink ink-g"
              x={HERO_SCENE.authorBox.x}
              y={HERO_SCENE.authorBox.y}
              width={HERO_SCENE.authorBox.width}
              height={HERO_SCENE.authorBox.height}
              rx={HERO_SCENE.authorBox.rx}
            />
            <circle
              className="ink ink-g"
              cx={HERO_SCENE.authorEyeL.cx}
              cy={HERO_SCENE.authorEyeL.cy}
              r={HERO_SCENE.authorEyeL.r}
            />
            <circle
              className="ink ink-g"
              cx={HERO_SCENE.authorEyeR.cx}
              cy={HERO_SCENE.authorEyeR.cy}
              r={HERO_SCENE.authorEyeR.r}
            />
            <path className="ink ink-g" d={HERO_SCENE.authorMouth} />
            <path className="ink ink-g" d={HERO_SCENE.authorAntenna} />
            <circle
              className="ink-fill"
              cx={HERO_SCENE.authorAntennaTip.cx}
              cy={HERO_SCENE.authorAntennaTip.cy}
              r={HERO_SCENE.authorAntennaTip.r}
              style={{ fill: 'var(--author)' }}
            />
            <path className="ink ink-g" d={HERO_SCENE.authorArm} />
            <path className="ink ink-g" d={HERO_SCENE.authorArrow} />
          </g>
          <text className="scene-cap a" x="100" y="190" textAnchor="middle">
            author · writes
          </text>
          <g className="glow">
            <rect
              className="ink ink-a"
              x={HERO_SCENE.diffCardBox.x}
              y={HERO_SCENE.diffCardBox.y}
              width={HERO_SCENE.diffCardBox.width}
              height={HERO_SCENE.diffCardBox.height}
              rx={HERO_SCENE.diffCardBox.rx}
            />
            <path className="ink ink-g no-draw" d={HERO_SCENE.diffLine1} style={{ opacity: 0.8 }} />
            <path className="ink ink-r no-draw" d={HERO_SCENE.diffLine2} style={{ opacity: 0.8 }} />
            <path className="ink ink-g no-draw" d={HERO_SCENE.diffLine3} style={{ opacity: 0.8 }} />
          </g>
          <path
            className="ink no-draw"
            strokeDasharray="4 7"
            d={HERO_SCENE.connector}
            style={{ opacity: 0.5 }}
          />
          <g className="glow">
            <rect
              className="ink ink-p"
              x={HERO_SCENE.criticBox.x}
              y={HERO_SCENE.criticBox.y}
              width={HERO_SCENE.criticBox.width}
              height={HERO_SCENE.criticBox.height}
              rx={HERO_SCENE.criticBox.rx}
            />
            <path className="ink ink-p" d={HERO_SCENE.criticBrow} />
            <circle
              className="ink ink-p"
              cx={HERO_SCENE.criticEyeL.cx}
              cy={HERO_SCENE.criticEyeL.cy}
              r={HERO_SCENE.criticEyeL.r}
            />
            <circle
              className="ink ink-p"
              cx={HERO_SCENE.criticEyeR.cx}
              cy={HERO_SCENE.criticEyeR.cy}
              r={HERO_SCENE.criticEyeR.r}
            />
            <path className="ink ink-p" d={HERO_SCENE.criticHandle} />
            <path className="ink ink-p" d={HERO_SCENE.criticMouth} />
          </g>
          <text className="scene-cap c" x="460" y="190" textAnchor="middle">
            critic · audits
          </text>
        </svg>
      </div>

      <p className="lede reveal">
        Every task and code change routes through the <b>.claude/</b> system. You name no agent. It
        reads the files you touched, picks the <b>author</b> that owns that area, runs an{' '}
        <b>independent critic</b> over the diff, pulls in a security or performance <b>secondary</b>{' '}
        when there&rsquo;s risk, writes tests, sweeps dead code, and lets a <b>steward</b> close the
        docs. Poke around below to watch it move.
      </p>

      <Divider d={DIVIDER_HERO} />
    </>
  );
}
