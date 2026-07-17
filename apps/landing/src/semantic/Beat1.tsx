import { beat1 } from "@/content/beat1";

export default function Beat1() {
  return (
    <section className="stage beat1">
      <div className="photo"></div><div className="grade"></div>
      <div className="inner">
        <h2 className="sr-only">{beat1.srHeading}</h2>
        <div className="section-label reveal">{beat1.sectionLabel}</div>
        <svg className="deco draw sm-hide" style={{ right: "4%", top: "-18px", width: "64px" }} viewBox="0 0 70 70" aria-hidden="true">
          <path pathLength="1" d="M46 6 A21 21 0 1 0 60 42 A17 17 0 0 1 46 6" />
          <path pathLength="1" d="M14 46 v12 M8 52 h12" style={{ strokeWidth: "2.4" }} />
          <path pathLength="1" d="M24 14 v8 M20 18 h8" style={{ strokeWidth: "2.2" }} />
        </svg>
        <svg className="deco draw sm-hide" style={{ left: "3%", top: "-12px", width: "56px" }} viewBox="0 0 60 60" aria-hidden="true">
          <path pathLength="1" d="M8 30 a22 22 0 1 0 44 0 a22 22 0 1 0 -44 0" />
          <path pathLength="1" d="M30 30 l9 -8 M30 30 l-13 3" style={{ strokeWidth: "2.6" }} />
          <path pathLength="1" d="M30 10 v4 M30 46 v4 M10 30 h4 M46 30 h4" style={{ strokeWidth: "2" }} />
        </svg>
        <svg className="deco draw sm-hide" style={{ right: "-6px", top: "44%", width: "48px" }} viewBox="0 0 52 52" aria-hidden="true">
          <path pathLength="1" d="M10 22 h26 l-3 22 h-20 Z" />
          <path pathLength="1" d="M36 26 q10 0 8 8 q-2 7 -10 6" style={{ strokeWidth: "2.6" }} />
          <path pathLength="1" d="M17 16 q-3 -5 0 -10 M25 15 q-3 -5 0 -10" style={{ strokeWidth: "2.2", opacity: ".8" }} />
        </svg>
        <svg className="deco draw sm-hide" style={{ left: "-6px", top: "44%", width: "34px" }} viewBox="0 0 36 36" aria-hidden="true"><path pathLength="1" d="M16 4 l7 2 5 -1 2 6 5 4 -4 4 1 6 -6 2 -2 5 -7 -1 -5 3 -4 -5 -6 -2 2 -6 -2 -5 5 -4 1 -5 Z" style={{ strokeWidth: "2.4" }} /><path pathLength="1" d="M12 12 l6 4 -2 6 7 3" style={{ strokeWidth: "1.8" }} /></svg>
        <div className="doodle beat1-dood reveal tap" data-scream="" data-voice={beat1.doodleVoice} data-lines={beat1.doodleLines} role="button" tabIndex={0} aria-label={beat1.doodleAria}>
          <div className="bubble"></div>
          <svg viewBox="0 0 320 240">
            <rect x="26" y="198" width="268" height="9" rx="3" className="dood-fill" style={{ fill: "#5a6273" }} />
            <rect x="186" y="166" width="74" height="34" rx="3" className="dood" style={{ fill: "#26303f" }} />
            <rect x="180" y="200" width="86" height="6" rx="2" className="dood-fill" style={{ fill: "#3a4250" }} />
            <circle className="dood" cx="108" cy="92" r="25" />
            <path className="dood" d="M95 90 q6 6 12 0" /><path className="dood" d="M111 90 q6 6 12 0" />
            <path className="dood" d="M95 112 q13 -9 26 0" />
            <ellipse className="dood-fill" cx="93" cy="116" rx="3" ry="5.5" style={{ fill: "#6cc6ff" }}><animate attributeName="cy" values="116;150;150;116" keyTimes="0;0.45;0.5;1" dur="2.6s" repeatCount="indefinite" /><animate attributeName="opacity" values="1;1;0;0;1" keyTimes="0;0.4;0.5;0.95;1" dur="2.6s" repeatCount="indefinite" /></ellipse>
            <path className="dood" d="M108 117 L108 200" />
            <path className="dood" d="M108 128 q-26 24 -42 70" />
            <path className="dood" d="M108 128 q44 20 70 70" />
          </svg>
        </div>
        <div className="tabs reveal">
          {beat1.screencaps.map((s) => (
            <span className="screencap" key={s}>{s}</span>
          ))}
        </div>
        <p className="thought reveal">{beat1.thought}</p>
        <p className="counter reveal" style={{ marginTop: "18px" }}>{beat1.counterA}<span data-to="247">0</span>{beat1.counterB}<span data-to="248">0</span>{beat1.counterC}</p>
        <p className="reveal" style={{ fontFamily: "var(--hand)", fontSize: "14px", color: "#8e9bb0", marginTop: "6px", fontStyle: "italic" }}>{beat1.italicLine}</p>
      </div>
    </section>
  );
}
