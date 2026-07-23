import { GITHUB_REPO, SPONSOR } from '@/lib/site-links';

// Body markup for /creature, converted 1:1 from the deleted
// src/content/creature/body.html. creature-0.js (the SVG film engine) and
// creature-1.js bind to this DOM entirely by id (#stage, #caps, #fig, #hud,
// #controls, #progress, #titlecard, #endcard, #play, #pauseBtn, ...) — the
// rendered DOM must stay byte-identical (ADR 0018). The root
// <div style={{display:'contents'}}> replaces the old RawHtml wrapper so the
// serialized DOM is unchanged.
//
// `hidden` (#endcard) and `inert` (#controls) are written as bare boolean
// props, NOT `="" `: React's boolean-attribute props (hidden/inert/disabled/…)
// treat an empty-string VALUE as falsy and drop the attribute entirely — only
// `true` (the bare-prop shorthand) serializes to the empty-string DOM
// attribute the original HTML has and creature-0.js's hasAttribute() checks
// expect.
//
// The endcard's "← back to the notebook" link is its own inline `.back`
// anchor, not the shared BackLink component (that one renders different text
// and isn't used on this page).
export function CreatureBody() {
  return (
    <div style={{ display: 'contents' }}>
      {/* static margin doodles (the notebook around the film) */}
      <div className="doodle" id="d-todo">
        todo:
        <br />
        1. get job
        <br />
        <span className="strike">2. ???</span>
        <br />
        3. feed creature (NO)
      </div>
      <div className="doodle" id="d-page">
        p.47 — the creature incident
      </div>
      <svg
        className="doodle"
        id="d-ring"
        width="90"
        height="80"
        viewBox="0 0 90 80"
        aria-hidden="true"
      >
        <ellipse
          cx="45"
          cy="40"
          rx="36"
          ry="28"
          fill="none"
          stroke="#1c1812"
          strokeWidth="5"
          opacity=".18"
        />
        <ellipse
          cx="47"
          cy="42"
          rx="33"
          ry="25"
          fill="none"
          stroke="#1c1812"
          strokeWidth="2.5"
          opacity=".12"
        />
      </svg>
      <svg
        className="doodle"
        id="d-ttt"
        width="84"
        height="84"
        viewBox="0 0 84 84"
        aria-hidden="true"
      >
        <g stroke="#1c1812" strokeWidth="2.6" strokeLinecap="round" fill="none" opacity=".6">
          <path d="M30 6 L28 78" />
          <path d="M56 4 L58 79" />
          <path d="M5 30 L79 28" />
          <path d="M6 56 L78 58" />
          <path d="M10 10 L24 24 M24 10 L10 24" />
          <path d="M36 12 L50 24 M50 10 L36 24" />
          <path d="M62 36 L76 50 M76 36 L62 50" />
          <path d="M12 62 L24 76 M24 62 L12 76" />
          <path d="M38 38 L50 50 M50 38 L38 50" />
        </g>
        <text x="6" y="82" fontFamily="Gloria Hallelujah,cursive" fontSize="9" fill="#e24b4a">
          nobody wins
        </text>
      </svg>

      <div id="frame">
        <div id="stagebox">
          <svg
            id="stage"
            viewBox="0 0 1200 675"
            preserveAspectRatio="xMidYMid meet"
            aria-hidden="true"
          >
            <defs>
              <filter id="boilA" x="-10%" y="-10%" width="120%" height="120%">
                <feTurbulence
                  type="fractalNoise"
                  baseFrequency="0.015"
                  numOctaves="2"
                  seed="1"
                  result="t"
                />
                <feDisplacementMap in="SourceGraphic" in2="t" scale="3.2" />
              </filter>
              <filter id="boilB" x="-10%" y="-10%" width="120%" height="120%">
                <feTurbulence
                  type="fractalNoise"
                  baseFrequency="0.02"
                  numOctaves="2"
                  seed="4"
                  result="t"
                />
                <feDisplacementMap in="SourceGraphic" in2="t" scale="4.2" />
              </filter>
              <filter id="boilC" x="-10%" y="-10%" width="120%" height="120%">
                <feTurbulence
                  type="fractalNoise"
                  baseFrequency="0.012"
                  numOctaves="2"
                  seed="7"
                  result="t"
                />
                <feDisplacementMap in="SourceGraphic" in2="t" scale="2.6" />
              </filter>
            </defs>
            <g id="frameline" />
            <g id="sceneRoot" />
          </svg>
          <div id="caps" aria-live="polite" aria-atomic="false" />
          <div id="fig" />
        </div>
      </div>

      <div id="hud" aria-hidden="true">
        <div id="hud1" />
        <div id="hud2" />
        <div id="hud3" />
      </div>

      <div id="controls" inert>
        <button id="pauseBtn" aria-label="pause">
          ⏸
        </button>
        <button id="speedBtn" aria-label="speed 1x">
          1×
        </button>
        <button id="muteBtn" aria-label="mute">
          🔊
        </button>
        <button id="skipBtn" aria-label="skip to next scene">
          ⏭
        </button>
        <button id="replayBtn" aria-label="replay from the start">
          ↺
        </button>
      </div>

      <div id="progress" aria-hidden="true">
        <div id="bar">
          <i id="barFill" />
        </div>
        <span id="plabel" />
      </div>

      <div id="titlecard" className="overlay">
        <h1>THE CREATURE</h1>
        <div className="tsub">a documentary about networking</div>
        <div className="tnote">based on a true story. unfortunately.</div>
        <button id="play" aria-label="play the film">
          <svg viewBox="0 0 120 120" aria-hidden="true">
            <path
              id="playCircle"
              d=""
              fill="none"
              stroke="var(--ink)"
              strokeWidth="4"
              strokeLinecap="round"
            />
            <path
              d="M49 39 L86 60 L49 81 Z"
              fill="var(--red)"
              stroke="var(--ink)"
              strokeWidth="3"
              strokeLinejoin="round"
            />
          </svg>
        </button>
        <div className="tmono">runtime ~2:40 · shorter than a take-home</div>
        <div className="tmono thint">
          sound on · space pause · 2× button · m mute · → skip · r replay
        </div>
      </div>

      <div
        id="endcard"
        className="overlay"
        hidden
        role="dialog"
        aria-modal="true"
        aria-labelledby="endcard-title"
      >
        <h2 id="endcard-title">THE END</h2>
        <p className="esub">the creature is fine. it lives in the app now. it answers to “send”.</p>
        <div className="tnote">no recruiters were harmed. several were automated.</div>
        <div className="tmono">
          rejections survived: 1,247 · dignity: rebooting · you: press send
        </div>
        <div className="erow">
          <button id="replay2" className="hbtn">
            ↺ watch it again
          </button>
          <a className="back" href="/">
            ← back to the notebook
          </a>
        </div>
      </div>
      <p
        id="sitefoot"
        style={{
          position: 'fixed',
          bottom: '0',
          left: '0',
          right: '0',
          zIndex: '6',
          textAlign: 'center',
          fontFamily: 'var(--mono)',
          fontSize: '12px',
          color: '#4a4030',
          padding: '4px 0 6px',
          background: 'rgba(244,236,220,.82)',
          pointerEvents: 'none',
        }}
      >
        <a
          href={SPONSOR}
          target="_blank"
          rel="noopener"
          style={{ color: '#4a4030', pointerEvents: 'auto' }}
        >
          ♥ sponsor
        </a>
        {' · '}
        <a href="/" style={{ color: '#4a4030', pointerEvents: 'auto' }}>
          home
        </a>
        {' · '}
        <a
          href={GITHUB_REPO}
          target="_blank"
          rel="noopener"
          style={{ color: '#4a4030', pointerEvents: 'auto' }}
        >
          GitHub
        </a>
      </p>
    </div>
  );
}
