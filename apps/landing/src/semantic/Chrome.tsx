import { chrome as c } from "@/content/chrome";

// Fixed chrome, transcribed 1:1 from landing/index.html. Server component:
// no interactivity here. legacy.ts binds behavior via these ids/classes.
export default function Chrome() {
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
          {c.loaderText}
        </div>
      </div>

      <div id="progress-wrap">
        <div id="progress"></div>
        <div id="progress-label">{c.progressLabel}</div>
      </div>
      <div id="odometer">{c.odometer}</div>
      <button
        id="sound-toggle"
        aria-label={c.soundLabel}
        title={c.soundLabel}
        aria-pressed="false"
      >
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

      <div id="cookie">
        {c.cookieText}
        <div>
          <button aria-label={c.cookieDismissAria}>{c.cookieDismiss}</button>
        </div>
      </div>
      <div id="slowdown">{c.slowdown}</div>
      <div id="jk">{c.jk}</div>
    </>
  );
}
