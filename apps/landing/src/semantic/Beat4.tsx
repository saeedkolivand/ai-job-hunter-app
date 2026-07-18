import { beat4 } from "@/content/beat4";

export default function Beat4() {
  return (
    <section className="stage beat4">
      <div className="photo" />
      <div className="grade" />
      <div className="sun" />
      <svg className="deco draw sm-hide" style={{ left: "14%", top: "13%", width: "90px" }} viewBox="0 0 90 40" aria-hidden="true">
        <path pathLength="1" d="M6 24 q9 -12 18 0 q9 -12 18 0" style={{ stroke: "#5a3610" }} />
        <path pathLength="1" d="M52 10 q7 -9 14 0 q7 -9 14 0" style={{ stroke: "#5a3610", strokeWidth: 2.4 }} />
      </svg>
      <svg className="deco draw sm-hide" style={{ left: "5%", top: "30%", width: "120px" }} viewBox="0 0 120 54" aria-hidden="true">
        <path pathLength="1" d="M16 42 q-12 -2 -8 -12 q-2 -12 12 -12 q4 -12 18 -8 q10 -8 20 0 q14 -4 16 8 q12 2 8 14 q2 10 -12 10 H24" style={{ stroke: "#7a4a14", strokeWidth: 2.8 }} />
      </svg>
      <svg className="deco draw sm-hide sun-rays" style={{ right: "calc(8% - 30px)", top: "calc(10% - 30px)", width: "150px" }} viewBox="0 0 150 150" aria-hidden="true">
        <path pathLength="1" d="M75 6 v14 M75 130 v14 M6 75 h14 M130 75 h14 M26 26 l10 10 M114 114 l10 10 M124 26 l-10 10 M36 114 l-10 10" style={{ stroke: "#b8770e", strokeWidth: 3 }} />
      </svg>
      <svg className="deco draw sm-hide" style={{ right: "14%", top: "38%", width: "90px" }} viewBox="0 0 110 50" aria-hidden="true">
        <path pathLength="1" d="M16 38 q-10 -2 -6 -10 q-2 -10 10 -10 q4 -10 16 -6 q10 -6 18 0 q12 -2 12 8 q10 2 6 12 q2 8 -10 8 H22" style={{ stroke: "#7a4a14", strokeWidth: 2.4 }} />
      </svg>
      <svg className="deco draw sm-hide" style={{ left: "10%", bottom: "8%", width: "44px" }} viewBox="0 0 44 64" aria-hidden="true">
        <path pathLength="1" d="M22 60 q2 -14 0 -26" style={{ stroke: "#2a6d2a", strokeWidth: 2.6 }} />
        <path pathLength="1" d="M22 40 q-10 6 -12 -2 M22 44 q10 4 12 -4" style={{ stroke: "#2a6d2a", strokeWidth: 2.2 }} />
        <path pathLength="1" d="M22 30 q-8 -2 -6 -8 q2 -6 8 -4 q2 -6 8 -2 q6 2 2 8 q4 6 -4 8 q-6 2 -8 -2 Z" style={{ stroke: "#a3541c", strokeWidth: 2.4 }} />
      </svg>
      <svg className="deco draw sm-hide" style={{ left: "34%", top: "8%", width: "54px" }} viewBox="0 0 60 26" aria-hidden="true">
        <path pathLength="1" d="M4 16 q5 -7 10 0 q5 -7 10 0" style={{ stroke: "#5a3610", strokeWidth: 2 }} />
        <path pathLength="1" d="M36 8 q4 -6 8 0 q4 -6 8 0" style={{ stroke: "#5a3610", strokeWidth: 1.8 }} />
      </svg>
      <svg className="stonks" viewBox="0 0 100 70">
        <path d="M6 64 L34 40 L52 50 L92 8" fill="none" stroke="#2a7d2a" strokeWidth="5" strokeLinecap="round" strokeLinejoin="round" />
        <path d="M74 8 L92 8 L92 26" fill="none" stroke="#2a7d2a" strokeWidth="5" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
      <div className="inner">
        <h2 className="sr-only">{beat4.srHeading}</h2>
        <div
          className="doodle beat4-dood reveal tap"
          data-scream=""
          data-voice="smug"
          data-lines={beat4.doodleLines}
          role="button"
          tabIndex={0}
          aria-label={beat4.doodleAria}
        >
          <div className="bubble" />
          <svg viewBox="0 0 280 200">
            <circle className="dood" cx="120" cy="74" r="26" />
            <rect x="104" y="68" width="34" height="9" rx="4" className="dood-fill" />
            <path className="dood" d="M138 72 l8 1" />
            <path className="dood" d="M106 84 q12 9 24 2" />
            <path className="dood" d="M120 100 q14 26 44 30" />
            <path className="dood" d="M120 112 q-30 6 -40 -18" />
            <path className="dood" d="M164 130 q22 2 30 -8" />
            <path className="dood" d="M164 130 q-6 24 8 44" />
            <path className="dood" d="M170 174 l22 -8" />
          </svg>
        </div>
        <p className="big reveal">{beat4.big}</p>
        <p className="counter reveal">
          {beat4.counter.p1}
          <span data-to="247">0</span>
          {beat4.counter.p2}
          <span data-to="247">0</span>
          {beat4.counter.p3}
          <span data-to="6">0</span>
        </p>
        <p className="line reveal">{beat4.line1}</p>
        <p className="line reveal">
          {beat4.line2a}
          <b>{beat4.line2b}</b>
        </p>
      </div>
    </section>
  );
}
