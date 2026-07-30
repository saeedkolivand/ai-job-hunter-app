import { GITHUB_REPO, SPONSOR } from '@/lib/site-links';

// #titlecard/#endcard/#sitefoot of /creature, split out of CreatureBody.tsx
// purely for file size — verbatim ported markup, no props. See
// CreatureBody.tsx for the shared conversion notes; creature-0.js binds to
// this DOM entirely by id (#titlecard, #play, #playCircle, #endcard,
// #replay2) — the rendered DOM must stay byte-identical (ADR 0018).
//
// `hidden` (#endcard) is written as a bare boolean prop, NOT `="" `: React's
// boolean-attribute props (hidden/inert/disabled/…) treat an empty-string
// VALUE as falsy and drop the attribute entirely — only `true` (the
// bare-prop shorthand) serializes to the empty-string DOM attribute the
// original HTML has and creature-0.js's hasAttribute() checks expect.
//
// The endcard's "← back to the notebook" link is its own inline `.back`
// anchor, not the shared BackLink component (that one renders different text
// and isn't used on this page).
export function Overlays() {
  return (
    <>
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
    </>
  );
}
