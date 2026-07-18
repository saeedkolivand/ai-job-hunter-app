import { features } from "@/content/features";

export default function Features() {
  return (
    <section className="stage features">
      <div className="inner">
        <svg className="deco draw sm-hide" style={{ left: "-10px", top: "-6px", width: "46px" }} viewBox="0 0 46 46" aria-hidden="true">
          <path pathLength="1" d="M23 3 v12 M23 31 v12 M3 23 h12 M31 23 h12" style={{ strokeWidth: 2.6 }} />
          <path pathLength="1" d="M9 9 l8 8 M29 29 l8 8 M37 9 l-8 8 M17 29 l-8 8" style={{ strokeWidth: 2.2 }} />
        </svg>
        <svg className="deco draw sm-hide" style={{ right: "-8px", top: "4px", width: "44px", transform: "rotate(8deg)" }} viewBox="0 0 46 60" aria-hidden="true">
          <path pathLength="1" d="M23 4 a14 14 0 0 1 8 25 q-3 3 -3 7 H18 q0 -4 -3 -7 a14 14 0 0 0 8 -25 Z" style={{ strokeWidth: 2.6 }} />
          <path pathLength="1" d="M18 42 h10 M19 47 h8 M21 52 h4" style={{ strokeWidth: 2.2 }} />
        </svg>
        <svg className="deco draw sm-hide" style={{ left: "10%", top: "172px", width: "50px" }} viewBox="0 0 52 52" aria-hidden="true">
          <path pathLength="1" d="M14 26 a12 12 0 1 0 24 0 a12 12 0 1 0 -24 0" />
          <path pathLength="1" d="M26 4 v7 M26 41 v7 M4 26 h7 M41 26 h7 M10 10 l5 5 M37 37 l5 5 M42 10 l-5 5 M15 37 l-5 5" style={{ strokeWidth: 2.6 }} />
          <path pathLength="1" d="M21 26 a5 5 0 1 0 10 0 a5 5 0 1 0 -10 0" style={{ strokeWidth: 2.2 }} />
        </svg>
        <svg className="deco draw sm-hide" style={{ right: "10%", top: "160px", width: "46px" }} viewBox="0 0 48 52" aria-hidden="true">
          <path pathLength="1" d="M4 8 h16 v16 H4 Z M6 16 l5 5 9 -11" style={{ strokeWidth: 2.4 }} />
          <path pathLength="1" d="M4 32 h16 v16 H4 Z M6 40 l5 5 9 -11" style={{ strokeWidth: 2.4 }} />
          <path pathLength="1" d="M28 14 h16 M28 22 h12 M28 38 h16 M28 46 h12" style={{ strokeWidth: 2.2 }} />
        </svg>
        <h2 className="reveal">{features.h2}</h2>
        <p className="lede reveal">{features.lede}</p>
        <svg className="crayon-arrow" viewBox="0 0 120 60" aria-hidden="true">
          <path d="M8 12 q40 36 90 30" fill="none" stroke="var(--ink)" strokeWidth="3" strokeLinecap="round" />
          <path d="M84 48 l14 -6 m-14 6 l8 -13" fill="none" stroke="var(--ink)" strokeWidth="3" strokeLinecap="round" />
        </svg>
        <div className="feat-grid">
          <div className="feat reveal"><h3>{features.c1t}</h3><p>{features.c1p}</p></div>
          <div className="feat reveal"><h3>{features.c2t}</h3><p>{features.c2p}</p></div>
          <div className="feat reveal"><h3>{features.c3t}</h3><p>{features.c3p}</p></div>
          <div className="feat reveal"><h3>{features.c4t}</h3><p>{features.c4p}</p></div>
          <div className="feat reveal"><h3>{features.c5t}</h3><p>{features.c5p}</p></div>
          <div className="feat reveal"><h3>{features.c6t}</h3><p>{features.c6p}</p></div>
          <div className="feat reveal"><h3>{features.c7t}</h3><p>{features.c7p}</p></div>
          <div className="feat reveal"><h3>{features.c8t}</h3><p>{features.c8p}</p></div>
          <div className="feat reveal"><h3>{features.c9t}</h3><p>{features.c9pA}<i>{features.c9pI}</i>{features.c9pB}</p></div>
          <div className="feat reveal"><h3>{features.c10t}</h3><p>{features.c10p}</p></div>
          <div className="feat reveal"><h3>{features.c11t}</h3><p>{features.c11pA}<a href={features.c11ChromeHref} target="_blank" rel="noopener noreferrer" style={{ color: "var(--red)" }}>{features.c11Chrome}</a>{features.c11Sep}<a href={features.c11FirefoxHref} target="_blank" rel="noopener noreferrer" style={{ color: "var(--red)" }}>{features.c11Firefox}</a>{features.c11pEnd}</p></div>
        </div>
      </div>
    </section>
  );
}
