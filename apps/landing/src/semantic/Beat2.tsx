import { beat2 } from "@/content/beat2";

export default function Beat2() {
  return (
    <section className="stage beat2">
      <div className="photo"></div><div className="grade"></div>
      <div className="inner">
        <h2 className="reveal">{beat2.h2}</h2>
        <div className="doodle beat2-dood reveal tap" data-scream="" data-flail="" data-voice={beat2.doodleVoice} data-lines={beat2.doodleLines} role="button" tabIndex={0} aria-label={beat2.doodleAria}>
          <div className="bubble"></div>
          <svg className="deco draw sm-hide" style={{ left: "-54px", top: "4px", width: "44px" }} viewBox="0 0 48 48" aria-hidden="true"><path pathLength="1" d="M42 10 C18 0 4 18 14 28 C22 36 36 30 31 19 C27 11 16 14 19 22" /></svg>
          <svg className="deco draw sm-hide" style={{ right: "-50px", top: "24px", width: "40px" }} viewBox="0 0 44 44" aria-hidden="true"><path pathLength="1" d="M8 34 q4 -16 12 -6 q3 -14 11 -4 q7 -9 9 3" style={{ strokeWidth: "2.6" }} /><path pathLength="1" d="M22 40 h.1 M30 41 h.1" style={{ strokeWidth: "4" }} /></svg>
          <svg className="deco draw" style={{ right: "-24px", top: "-26px", width: "34px" }} viewBox="0 0 40 40" aria-hidden="true"><path pathLength="1" d="M8 6 l4 16 M13 30 h.1" style={{ strokeWidth: "3.4" }} /><path pathLength="1" d="M22 12 q-1 -8 7 -8 q8 0 7 8 q-1 6 -7 8 v4 M29 32 h.1" style={{ strokeWidth: "3" }} /></svg>
          <svg viewBox="0 0 240 220">
            <circle className="dood" cx="120" cy="80" r="24" />
            <path className="dood" d="M112 60 L108 48" /><path className="dood" d="M120 58 L120 46" /><path className="dood" d="M128 60 L132 48" /><path className="dood" d="M104 66 L92 60" /><path className="dood" d="M136 66 L148 60" />
            <circle className="dood-fill" cx="113" cy="80" r="2.4" /><circle className="dood-fill" cx="127" cy="80" r="2.4" />
            <path className="dood" d="M106 72 L116 76" /><path className="dood" d="M134 72 L124 76" />
            <ellipse className="dood" cx="120" cy="94" rx="5" ry="6" />
            <ellipse className="dood-fill" cx="146" cy="78" rx="2.6" ry="3.8" style={{ fill: "#6cc6ff" }} />
            <polyline className="dood" points="120,108 96,108 104,120 86,124" />
            <polyline className="dood" points="120,108 144,108 136,120 154,124" />
            <path className="dood" d="M120 108 L120 150" />
            <path className="dood" d="M120 150 L100 188" /><path className="dood" d="M120 150 L140 188" />
          </svg>
        </div>
        <div className="swarm reveal" id="swarm">
          {beat2.chips.map((c) => (
            <span className="chip x" key={c}>{c}</span>
          ))}<span className="chip">{beat2.chipsMore}</span>
        </div>
        <p className="tag reveal" style={{ margin: "6px auto 26px", maxWidth: "42ch", color: "#d1c6e6" }}>{beat2.tagA}<i>{beat2.tagItalic}</i></p>

        <svg className="deco draw sm-hide" style={{ left: "3%", top: "14%", width: "46px", transform: "rotate(-14deg)" }} viewBox="0 0 44 56" aria-hidden="true"><path pathLength="1" d="M6 6 h30 v44 h-30 Z" /><path pathLength="1" d="M12 16 h18 M12 24 h18 M12 32 h12" style={{ strokeWidth: "2" }} /><path pathLength="1" d="M26 38 l8 8 M34 38 l-8 8" style={{ stroke: "#e24b4a", strokeWidth: "2.6" }} /></svg>
        <svg className="deco draw sm-hide" style={{ right: "3%", top: "19%", width: "42px", transform: "rotate(12deg)" }} viewBox="0 0 44 56" aria-hidden="true"><path pathLength="1" d="M6 6 h30 v44 h-30 Z" /><path pathLength="1" d="M12 16 h18 M12 24 h16 M12 32 h18 M12 40 h10" style={{ strokeWidth: "2" }} /></svg>
        <div className="collage">
          <div className="card tilt-l reveal">
            <div className="blackhole">
              <span className="yell">{beat2.blackholeYell}</span>
              <span style={{ marginTop: "6px" }}>{beat2.blackholeMain}</span>
              <span className="echo">{beat2.blackholeEcho}</span>
            </div>
            <span className="ct" style={{ textAlign: "center", display: "block", marginTop: "8px" }}>{beat2.blackholeStatus}</span>
          </div>

          <div className="card tilt-r reveal">
            <span className="ct">{beat2.atsTitle}</span>
            <div className="ats">
              <svg viewBox="0 0 80 80"><rect x="10" y="20" width="60" height="46" rx="10" className="dood" style={{ fill: "#2a1f1f", stroke: "#e24b4a" }} /><circle cx="30" cy="36" r="9" fill="#fff" stroke="#e24b4a" strokeWidth="2" /><circle cx="52" cy="36" r="9" fill="#fff" stroke="#e24b4a" strokeWidth="2" /><circle cx="32" cy="38" r="3" fill="#111" /><circle cx="54" cy="34" r="3" fill="#111" /><path d="M24 54 q16 8 32 0" fill="none" stroke="#e24b4a" strokeWidth="3" strokeLinecap="round" /><path d="M26 54 l4 6 m6 -6 l3 7 m6 -7 l4 6 m6 -6 l3 7" stroke="#e24b4a" strokeWidth="2" /></svg>
              <span><b>{beat2.atsBold}</b><br />{beat2.atsSub}</span>
            </div>
          </div>

          <div className="card tilt-s reveal">
            <span className="ct">{beat2.recruitersTitle}</span>
            <div className="npcs">
              {beat2.npcs.map((n) => (
                <div className="npc" key={n}><svg viewBox="0 0 40 46"><circle cx="20" cy="14" r="10" fill="none" stroke="#8e80a4" strokeWidth="2.4" /><path d="M8 44 q12 -16 24 0" fill="none" stroke="#8e80a4" strokeWidth="2.4" /><path d="M14 14 h3 m6 0 h3" stroke="#8e80a4" strokeWidth="2" /><path d="M15 19 q5 3 10 0" stroke="#8e80a4" strokeWidth="2" fill="none" /></svg>{n}</div>
              ))}
            </div>
          </div>

          <div className="card tilt-l reveal">
            <span className="ct">{beat2.linkedinTitle}</span>
            <div className="feed"><div className="scroller">
              {beat2.feed.map((p, i) => (
                <p key={`a${i}`}>{p}</p>
              ))}
              {beat2.feed.map((p, i) => (
                <p key={`b${i}`}>{p}</p>
              ))}
            </div></div>
            <span className="ct" style={{ marginTop: "8px" }}>{beat2.linkedinGhost}</span>
          </div>
        </div>

        <svg className="deco draw sm-hide" style={{ left: "8%", bottom: "24px", width: "36px" }} viewBox="0 0 44 60" aria-hidden="true"><path pathLength="1" d="M22 4 q-9 10 0 18 q9 8 0 18 v10" /><path pathLength="1" d="M22 50 l-7 -8 M22 50 l7 -8" /></svg>
        <p className="counter reveal">{beat2.counterA}<span data-to="1000">0</span>{beat2.counterB}</p>
      </div>
    </section>
  );
}
