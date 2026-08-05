// Hero <section> of / (home), split out of HomeBody.tsx purely for file
// size — verbatim ported markup, no props. See HomeBody.tsx for the shared
// conversion notes; public/scripts/home-0.js binds to #hero-dood and
// .stage/.reveal by id/class (ADR-0018).
export function Hero() {
  return (
    <section className="stage hero">
      <div className="photo" />
      <div className="grade" />
      <div className="inner">
        <div className="kicker reveal">a real desktop app · also a cry for help</div>
        <svg
          className="deco draw sm-hide"
          style={{ right: '0', top: '-6px', width: 'clamp(90px,11vw,140px)' }}
          viewBox="0 0 140 90"
          aria-hidden="true"
        >
          <path
            pathLength="1"
            className="dashed"
            strokeDasharray=".045 .04"
            d="M6 86 Q34 72 46 50 Q56 32 82 26"
            style={{ strokeWidth: '2.4', opacity: '.7' }}
          />
          <path pathLength="1" d="M84 30 L126 14 L102 44 L96 34 Z" />
          <path pathLength="1" d="M96 34 L98 48 L106 39" />
        </svg>
        <svg
          className="deco draw sm-hide"
          style={{ left: '2%', top: '0', width: '80px' }}
          viewBox="0 0 90 44"
          aria-hidden="true"
        >
          <path
            pathLength="1"
            className="dashed"
            strokeDasharray=".05 .05"
            d="M88 4 Q52 12 18 30"
            style={{ strokeWidth: '2.2', opacity: '.65' }}
          />
          <path
            pathLength="1"
            d="M12 26 v12 M6 32 h12 M8 28 l8 8 M16 28 l-8 8"
            style={{ strokeWidth: '2.2' }}
          />
        </svg>
        <svg
          className="deco draw sm-hide"
          style={{ left: '-14px', top: '42%', width: '30px' }}
          viewBox="0 0 30 30"
          aria-hidden="true"
        >
          <path
            pathLength="1"
            d="M15 3 v9 M15 18 v9 M3 15 h9 M18 15 h9"
            style={{ strokeWidth: '2.4' }}
          />
        </svg>
        <svg
          className="deco draw sm-hide"
          style={{ right: '-12px', top: '56%', width: '22px' }}
          viewBox="0 0 24 24"
          aria-hidden="true"
        >
          <path
            pathLength="1"
            d="M12 2 v8 M12 14 v8 M2 12 h8 M14 12 h8"
            style={{ strokeWidth: '2.2' }}
          />
        </svg>
        <svg
          className="deco draw sm-hide"
          style={{ left: '5%', bottom: '6px', width: '46px' }}
          viewBox="0 0 50 50"
          aria-hidden="true"
        >
          <path
            d="M22 5 l9 3 7 -1 3 8 7 5 -5 6 1 8 -8 3 -3 7 -9 -1 -7 4 -5 -7 -8 -3 3 -8 -3 -7 7 -5 1 -7 Z"
            pathLength="1"
          />
          <path pathLength="1" d="M17 16 l8 6 -3 8 9 4" style={{ strokeWidth: '2' }} />
        </svg>
        <div
          className="doodle hero-dood reveal tap"
          id="hero-dood"
          role="button"
          tabIndex={0}
          aria-label="poke the guy"
        >
          <div className="bubble" />
          <div className="dontclick" aria-hidden="true">
            <div className="dc-inner">
              <svg className="dc-arrow" viewBox="0 0 50 44">
                <path
                  d="M46 10 Q22 10 8 32"
                  fill="none"
                  stroke="#ffe27a"
                  strokeWidth="3.6"
                  strokeLinecap="round"
                />
                <path
                  d="M8 32 l13 -2 M8 32 l4 -13"
                  fill="none"
                  stroke="#ffe27a"
                  strokeWidth="3.6"
                  strokeLinecap="round"
                />
              </svg>
              <span className="dc-text">don't click me</span>
            </div>
          </div>
          <svg viewBox="0 0 200 175">
            <circle className="dood" cx="100" cy="66" r="26" />
            <path className="dood" d="M86 64 q6 6 12 0" />
            <path className="dood" d="M102 64 q6 6 12 0" />
            <path className="dood" d="M86 88 q14 -10 28 0" />
            <ellipse
              className="dood-fill"
              cx="84"
              cy="92"
              rx="3"
              ry="5"
              style={{ fill: '#6cc6ff' }}
            />
            <path className="dood" d="M100 92 L100 138" />
            <path className="dood" d="M100 104 q-22 12 -30 34" />
            <path className="dood" d="M100 104 q22 12 30 34" />
            <path className="dood" d="M100 138 L82 172" />
            <path className="dood" d="M100 138 L118 172" />
          </svg>
        </div>
        <h1 className="reveal">
          Job hunting broke me.
          <br />
          So I built a{' '}
          <span className="ul">
            robot
            <svg className="draw" viewBox="0 0 120 14" aria-hidden="true">
              <path pathLength="1" d="M4 8 Q34 3 62 6 T116 6" />
              <path pathLength="1" d="M14 12 Q52 8 104 10" />
            </svg>
          </span>{' '}
          to do it.
        </h1>
        <p className="sub reveal">
          AI Job Hunter covers 24 job boards, writes your cover letters, and drafts your
          applications while you dissociate — you just press send. Runs fully offline.{' '}
          <b>Built by a guy who is, himself, still unemployed.</b>
        </p>
        <div className="scrollhint reveal">
          scroll to watch a man fall apart
          <span className="arr">↓</span>
        </div>
        <a className="filmhint reveal" href="/creature">
          ▶ or don't scroll — watch THE CREATURE. he summons a tiny recruiter. it grows. (2:40)
        </a>
        <a className="filmhint reveal" href="/how-it-works">
          skeptical? → see how it works, end to end
        </a>
        <a className="filmhint reveal" href="/download">
          already sold? → just take the app
        </a>
      </div>
    </section>
  );
}
