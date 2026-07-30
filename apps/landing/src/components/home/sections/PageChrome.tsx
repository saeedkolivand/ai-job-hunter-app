import type { ReactNode } from 'react';

// Loader/progress/odometer/sound-toggle/slowdown/jk chrome for / (home),
// split out of HomeBody.tsx purely for file size — verbatim ported markup,
// no props beyond `children` (the CookieGag slot: it renders between the
// sound toggle and #slowdown in the original DOM order, so the shell passes
// <CookieGag /> in here rather than PageChrome importing/rendering it
// itself — CookieGag's import/usage stays in HomeBody.tsx per the split
// plan). See HomeBody.tsx for the shared conversion notes; public/scripts/
// home-0.js binds to #journey/#journey-path/#journey-tip/#journey-path-2/
// #journey-tip-2, #loader/#loader-text, #progress-wrap/#progress/
// #progress-label, #odometer, #sound-toggle, #slowdown, #jk by id
// (ADR-0018).
export function PageChrome({ children }: { children?: ReactNode }) {
  return (
    <>
      <div id="journey" aria-hidden="true">
        <svg>
          <path id="journey-path" d="M0 0" />
          <g id="journey-tip">
            <path d="M-14 -8 L2 0 L-14 8" />
          </g>
          <path id="journey-path-2" d="M0 0" />
          <g id="journey-tip-2">
            <path d="M-14 -8 L2 0 L-14 8" />
          </g>
        </svg>
      </div>

      <div id="loader">
        <div className="lt" id="loader-text">
          summoning a sad little man…
        </div>
      </div>

      <div id="progress-wrap">
        <div id="progress" />
        <div id="progress-label">job search: 0% complete · est. time remaining: ∞</div>
      </div>
      <div id="odometer">rejections survived: 1,247</div>
      <button id="sound-toggle" aria-label="mute the guy" title="mute the guy" aria-pressed="false">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M3 9 h4 l5 -4 v14 l-5 -4 h-4 z" fill="currentColor" />
          <g
            className="waves"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.8"
            strokeLinecap="round"
          >
            <path d="M15 9.5 q2.4 2.5 0 5" />
            <path d="M17.6 7.5 q4 4.5 0 9" />
          </g>
          <line
            className="slash"
            x1="14.5"
            y1="6.5"
            x2="21.5"
            y2="17.5"
            stroke="currentColor"
            strokeWidth="1.8"
            strokeLinecap="round"
          />
        </svg>
      </button>

      {children}

      <div id="slowdown">slow down — you're making it worse</div>
      <div id="jk">just kidding.</div>
    </>
  );
}
