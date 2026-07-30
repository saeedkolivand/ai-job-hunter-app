// Static margin doodles (the notebook around the film) of /creature, split
// out of CreatureBody.tsx purely for file size — verbatim ported markup, no
// props. See CreatureBody.tsx for the shared conversion notes; these are
// decorative only and are not bound by creature-0.js/creature-1.js.
export function Doodles() {
  return (
    <>
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
    </>
  );
}
