// Beat 3 — DEEP FRIED <section> of / (home), split out of HomeBeats.tsx
// purely for file size — verbatim ported markup, no props. See
// HomeBody.tsx for the shared conversion notes; public/scripts/home-0.js
// binds to .beat3-dood, .dialog, .dq, .y2 and
// [data-scream]/[data-voice]/[data-lines] by class/data-* (ADR-0018).
export function Beat3() {
  return (
    <section className="stage beat3">
      <div className="photo" />
      <div className="grade" />
      <div className="inner">
        <div
          className="doodle beat3-dood reveal tap"
          data-scream=""
          data-voice="fried"
          data-lines="IT WRITES IT FOR ME|I HAVE THE POWER|NO MORE COVER LETTERS|unlimited POWER|YESSS"
          role="button"
          tabIndex={0}
          aria-label="poke the fried guy"
        >
          <div className="bubble" />
          <svg viewBox="0 0 260 240">
            <circle className="dood" cx="130" cy="104" r="48" style={{ strokeWidth: '4' }} />
            <circle cx="110" cy="96" r="19" fill="#fff" stroke="#000" strokeWidth="3" />
            <circle cx="150" cy="96" r="19" fill="#fff" stroke="#000" strokeWidth="3" />
            <path
              d="M96 92 l8 6 m6 -10 l5 9 M152 92 l8 6 m-2 -10 l-3 10"
              stroke="#e24b4a"
              strokeWidth="1.6"
            />
            <circle cx="112" cy="98" r="4.5" fill="#000" />
            <circle cx="148" cy="98" r="4.5" fill="#000" />
            <path
              className="dood"
              d="M86 60 l-8 -12 M120 50 l-2 -14 M160 58 l10 -12"
              style={{ strokeWidth: '3' }}
            />
            <path
              d="M96 132 q34 40 68 0 q-34 14 -68 0 Z"
              fill="#fff"
              stroke="#000"
              strokeWidth="3.4"
            />
            <path
              d="M108 138 v14 M124 141 v15 M140 141 v15 M152 138 v13"
              stroke="#000"
              strokeWidth="2.4"
            />
            <path className="dood" d="M130 152 L130 196" style={{ strokeWidth: '4' }} />
            <path
              className="dood"
              d="M130 164 L96 150 M130 164 L164 150"
              style={{ strokeWidth: '4' }}
            />
            <path
              className="dood"
              d="M130 196 L108 232 M130 196 L152 232"
              style={{ strokeWidth: '4' }}
            />
          </svg>
        </div>
        <svg
          className="deco draw sm-hide"
          style={{
            left: '-2%',
            top: '30%',
            width: '50px',
            transform: 'rotate(-10deg)',
            filter: 'drop-shadow(2px 2px 0 #ff00d4)',
          }}
          viewBox="0 0 50 84"
          aria-hidden="true"
        >
          <path
            pathLength="1"
            d="M32 4 L12 40 H24 L8 80 L42 34 H28 L44 4 Z"
            style={{ strokeWidth: '3.4' }}
          />
        </svg>
        <svg
          className="deco draw sm-hide"
          style={{
            right: '-2%',
            top: '22%',
            width: '62px',
            transform: 'rotate(8deg)',
            filter: 'drop-shadow(-2px 2px 0 #00fff0)',
          }}
          viewBox="0 0 64 64"
          aria-hidden="true"
        >
          <path
            pathLength="1"
            d="M32 4 L37 24 L56 12 L43 30 L62 36 L41 38 L48 58 L32 44 L16 58 L23 38 L2 36 L21 30 L8 12 L27 24 Z"
            style={{ strokeWidth: '3' }}
          />
        </svg>
        <svg
          className="deco draw sm-hide"
          style={{
            left: '8%',
            top: '64%',
            width: '40px',
            filter: 'drop-shadow(1px 2px 0 #00fff0)',
          }}
          viewBox="0 0 40 56"
          aria-hidden="true"
        >
          <path
            pathLength="1"
            d="M20 52 q-14 -4 -12 -18 q1 -8 8 -12 q-2 8 4 10 q-2 -12 8 -20 q-1 10 6 14 q5 4 4 12 q-2 12 -18 14 Z"
            style={{ strokeWidth: '3' }}
          />
        </svg>
        <svg
          className="deco draw sm-hide"
          style={{
            right: '10%',
            top: '66%',
            width: '30px',
            transform: 'rotate(14deg)',
            filter: 'drop-shadow(1px 2px 0 #ff00d4)',
          }}
          viewBox="0 0 30 50"
          aria-hidden="true"
        >
          <path
            pathLength="1"
            d="M19 2 L8 24 H15 L6 48 L26 20 H17 L25 2 Z"
            style={{ strokeWidth: '2.8' }}
          />
        </svg>
        <svg
          className="deco draw"
          style={{ left: '16%', top: '10%', width: '26px' }}
          viewBox="0 0 26 26"
          aria-hidden="true"
        >
          <path
            pathLength="1"
            d="M13 2 v9 M13 15 v9 M2 13 h9 M15 13 h9"
            style={{ strokeWidth: '2.6' }}
          />
        </svg>
        <svg
          className="deco draw"
          style={{ right: '16%', top: '8%', width: '20px' }}
          viewBox="0 0 22 22"
          aria-hidden="true"
        >
          <path
            pathLength="1"
            d="M11 2 v7 M11 13 v7 M2 11 h7 M13 11 h7"
            style={{ strokeWidth: '2.4' }}
          />
        </svg>
        <h2 className="fried huge reveal">
          WAIT.
          <br />
          IT DOES
          <br />
          EVERYTHING ELSE.
        </h2>
        <div className="fried mid reveal">it finds them. it writes them. you press send.</div>
        <p className="fried-line reveal">
          it <b>HITS 24 boards</b> while you do NOTHING. it <b>writes the cover letter</b> — the one
          you CRIED over. GONE. it <b>scores your résumé</b> so the regex blob can't hurt you
          anymore. <b>semantic matching????</b> it KNOWS which jobs want you. it knows things you
          don't. the one thing it <b>won't</b> do? press submit. that terror stays yours.
        </p>
        <div className="dialog reveal" role="region" aria-label="secret confirmation">
          <span className="dq" aria-live="polite" aria-atomic="true">
            Are you sure?
          </span>
          <br />
          <span style={{ fontSize: '12px', color: '#444' }}>
            (you were not supposed to find this)
          </span>
          <div className="btns">
            <button>yes</button>
            <button className="y2">YES</button>
          </div>
        </div>
      </div>
    </section>
  );
}
