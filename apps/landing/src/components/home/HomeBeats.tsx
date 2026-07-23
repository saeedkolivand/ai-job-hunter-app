// beat1-4 sections of the home page — split out of HomeBody.tsx purely for
// file size (a mechanical 1:1 conversion of the four "BEAT" <section>s in the
// deleted src/content/home/body.html; no props). See HomeBody.tsx for the
// shared conversion notes and the ADR-0018 DOM-fidelity constraint that
// public/scripts/home-0.js depends on (poke-a-doodle via
// [data-scream]/[data-voice]/[data-lines], counters via [data-to], scroll-driven
// --p/--c scrubbing on .stage/.reveal).
export function HomeBeats() {
  return (
    <>
      {/* ============ BEAT 1 — SLUMP ============ */}
      <section className="stage beat1">
        <div className="photo" />
        <div className="grade" />
        <div className="inner">
          <h2 className="sr-only">the slump</h2>
          <div className="section-label reveal">2:47 AM</div>
          <svg
            className="deco draw sm-hide"
            style={{ right: '4%', top: '-18px', width: '64px' }}
            viewBox="0 0 70 70"
            aria-hidden="true"
          >
            <path pathLength="1" d="M46 6 A21 21 0 1 0 60 42 A17 17 0 0 1 46 6" />
            <path pathLength="1" d="M14 46 v12 M8 52 h12" style={{ strokeWidth: '2.4' }} />
            <path pathLength="1" d="M24 14 v8 M20 18 h8" style={{ strokeWidth: '2.2' }} />
          </svg>
          <svg
            className="deco draw sm-hide"
            style={{ left: '3%', top: '-12px', width: '56px' }}
            viewBox="0 0 60 60"
            aria-hidden="true"
          >
            <path pathLength="1" d="M8 30 a22 22 0 1 0 44 0 a22 22 0 1 0 -44 0" />
            <path pathLength="1" d="M30 30 l9 -8 M30 30 l-13 3" style={{ strokeWidth: '2.6' }} />
            <path
              pathLength="1"
              d="M30 10 v4 M30 46 v4 M10 30 h4 M46 30 h4"
              style={{ strokeWidth: '2' }}
            />
          </svg>
          <svg
            className="deco draw sm-hide"
            style={{ right: '-6px', top: '44%', width: '48px' }}
            viewBox="0 0 52 52"
            aria-hidden="true"
          >
            <path pathLength="1" d="M10 22 h26 l-3 22 h-20 Z" />
            <path pathLength="1" d="M36 26 q10 0 8 8 q-2 7 -10 6" style={{ strokeWidth: '2.6' }} />
            <path
              pathLength="1"
              d="M17 16 q-3 -5 0 -10 M25 15 q-3 -5 0 -10"
              style={{ strokeWidth: '2.2', opacity: '.8' }}
            />
          </svg>
          <svg
            className="deco draw sm-hide"
            style={{ left: '-6px', top: '44%', width: '34px' }}
            viewBox="0 0 36 36"
            aria-hidden="true"
          >
            <path
              pathLength="1"
              d="M16 4 l7 2 5 -1 2 6 5 4 -4 4 1 6 -6 2 -2 5 -7 -1 -5 3 -4 -5 -6 -2 2 -6 -2 -5 5 -4 1 -5 Z"
              style={{ strokeWidth: '2.4' }}
            />
            <path pathLength="1" d="M12 12 l6 4 -2 6 7 3" style={{ strokeWidth: '1.8' }} />
          </svg>
          <div
            className="doodle beat1-dood reveal tap"
            data-scream=""
            data-voice="sad"
            data-lines="...hi.|please hire me|i haven't slept|anyone? hello?"
            role="button"
            tabIndex={0}
            aria-label="poke the sad guy"
          >
            <div className="bubble" />
            <svg viewBox="0 0 320 240">
              <rect
                x="26"
                y="198"
                width="268"
                height="9"
                rx="3"
                className="dood-fill"
                style={{ fill: '#5a6273' }}
              />
              <rect
                x="186"
                y="166"
                width="74"
                height="34"
                rx="3"
                className="dood"
                style={{ fill: '#26303f' }}
              />
              <rect
                x="180"
                y="200"
                width="86"
                height="6"
                rx="2"
                className="dood-fill"
                style={{ fill: '#3a4250' }}
              />
              <circle className="dood" cx="108" cy="92" r="25" />
              <path className="dood" d="M95 90 q6 6 12 0" />
              <path className="dood" d="M111 90 q6 6 12 0" />
              <path className="dood" d="M95 112 q13 -9 26 0" />
              <ellipse
                className="dood-fill"
                cx="93"
                cy="116"
                rx="3"
                ry="5.5"
                style={{ fill: '#6cc6ff' }}
              >
                <animate
                  attributeName="cy"
                  values="116;150;150;116"
                  keyTimes="0;0.45;0.5;1"
                  dur="2.6s"
                  repeatCount="indefinite"
                />
                <animate
                  attributeName="opacity"
                  values="1;1;0;0;1"
                  keyTimes="0;0.4;0.5;0.95;1"
                  dur="2.6s"
                  repeatCount="indefinite"
                />
              </ellipse>
              <path className="dood" d="M108 117 L108 200" />
              <path className="dood" d="M108 128 q-26 24 -42 70" />
              <path className="dood" d="M108 128 q44 20 70 70" />
            </svg>
          </div>
          <div className="tabs reveal">
            <span className="screencap">
              cover_letter_FINAL_v9.docx — "please. please. please. please. pl—"
            </span>{' '}
            <span className="screencap">
              draft: "I have always dreamed of working at [COMPANY NAME]."
            </span>{' '}
            <span className="screencap">
              rejections (final)(final2)(FINAL_real)(use_this_one).pdf
            </span>{' '}
            <span className="screencap">tab: how to answer "what's your biggest weakness"</span>{' '}
            <span className="screencap">tab: is it normal to cry on a tuesday</span>
          </div>
          <p className="thought reveal">"maybe i'll hear back monday"</p>
          <p className="counter reveal" style={{ marginTop: '18px' }}>
            applications submitted: <span data-to="247">0</span> · unfortunatelies received:{' '}
            <span data-to="248">0</span> · est. callback: ∞
          </p>
          <p
            className="reveal"
            style={{
              fontFamily: 'var(--hand)',
              fontSize: '14px',
              color: '#8e9bb0',
              marginTop: '6px',
              fontStyle: 'italic',
            }}
          >
            the 248th wasn't a job. it was your crush. we don't talk about it.
          </p>
        </div>
      </section>

      {/* ============ BEAT 2 — DESCENT ============ */}
      <section className="stage beat2">
        <div className="photo" />
        <div className="grade" />
        <div className="inner">
          <h2 className="reveal">the descent</h2>
          <div
            className="doodle beat2-dood reveal tap"
            data-scream=""
            data-flail=""
            data-voice="frazzled"
            data-lines="WHY won't they reply|i can't keep up|so many tabs|make it STOP|aaaaaa"
            role="button"
            tabIndex={0}
            aria-label="poke the frazzled guy"
          >
            <div className="bubble" />
            <svg
              className="deco draw sm-hide"
              style={{ left: '-54px', top: '4px', width: '44px' }}
              viewBox="0 0 48 48"
              aria-hidden="true"
            >
              <path
                pathLength="1"
                d="M42 10 C18 0 4 18 14 28 C22 36 36 30 31 19 C27 11 16 14 19 22"
              />
            </svg>
            <svg
              className="deco draw sm-hide"
              style={{ right: '-50px', top: '24px', width: '40px' }}
              viewBox="0 0 44 44"
              aria-hidden="true"
            >
              <path
                pathLength="1"
                d="M8 34 q4 -16 12 -6 q3 -14 11 -4 q7 -9 9 3"
                style={{ strokeWidth: '2.6' }}
              />
              <path pathLength="1" d="M22 40 h.1 M30 41 h.1" style={{ strokeWidth: '4' }} />
            </svg>
            <svg
              className="deco draw"
              style={{ right: '-24px', top: '-26px', width: '34px' }}
              viewBox="0 0 40 40"
              aria-hidden="true"
            >
              <path pathLength="1" d="M8 6 l4 16 M13 30 h.1" style={{ strokeWidth: '3.4' }} />
              <path
                pathLength="1"
                d="M22 12 q-1 -8 7 -8 q8 0 7 8 q-1 6 -7 8 v4 M29 32 h.1"
                style={{ strokeWidth: '3' }}
              />
            </svg>
            <svg viewBox="0 0 240 220">
              <circle className="dood" cx="120" cy="80" r="24" />
              <path className="dood" d="M112 60 L108 48" />
              <path className="dood" d="M120 58 L120 46" />
              <path className="dood" d="M128 60 L132 48" />
              <path className="dood" d="M104 66 L92 60" />
              <path className="dood" d="M136 66 L148 60" />
              <circle className="dood-fill" cx="113" cy="80" r="2.4" />
              <circle className="dood-fill" cx="127" cy="80" r="2.4" />
              <path className="dood" d="M106 72 L116 76" />
              <path className="dood" d="M134 72 L124 76" />
              <ellipse className="dood" cx="120" cy="94" rx="5" ry="6" />
              <ellipse
                className="dood-fill"
                cx="146"
                cy="78"
                rx="2.6"
                ry="3.8"
                style={{ fill: '#6cc6ff' }}
              />
              <polyline className="dood" points="120,108 96,108 104,120 86,124" />
              <polyline className="dood" points="120,108 144,108 136,120 154,124" />
              <path className="dood" d="M120 108 L120 150" />
              <path className="dood" d="M120 150 L100 188" />
              <path className="dood" d="M120 150 L140 188" />
            </svg>
          </div>
          <div className="swarm reveal" id="swarm">
            <span className="chip x">LinkedIn</span>
            <span className="chip x">Indeed</span>
            <span className="chip x">Glassdoor</span>
            <span className="chip x">StepStone</span>
            <span className="chip x">Xing</span>
            <span className="chip x">Greenhouse</span>
            <span className="chip x">Lever</span>
            <span className="chip x">Workday</span>
            <span className="chip">+16 more</span>
          </div>
          <p
            className="tag reveal"
            style={{ margin: '6px auto 26px', maxWidth: '42ch', color: '#d1c6e6' }}
          >
            entry level: 5 yrs experience required · easy apply (47 fields) · create a Workday
            account <i>(for the 9th time)</i>
          </p>

          <svg
            className="deco draw sm-hide"
            style={{ left: '3%', top: '14%', width: '46px', transform: 'rotate(-14deg)' }}
            viewBox="0 0 44 56"
            aria-hidden="true"
          >
            <path pathLength="1" d="M6 6 h30 v44 h-30 Z" />
            <path
              pathLength="1"
              d="M12 16 h18 M12 24 h18 M12 32 h12"
              style={{ strokeWidth: '2' }}
            />
            <path
              pathLength="1"
              d="M26 38 l8 8 M34 38 l-8 8"
              style={{ stroke: '#e24b4a', strokeWidth: '2.6' }}
            />
          </svg>
          <svg
            className="deco draw sm-hide"
            style={{ right: '3%', top: '19%', width: '42px', transform: 'rotate(12deg)' }}
            viewBox="0 0 44 56"
            aria-hidden="true"
          >
            <path pathLength="1" d="M6 6 h30 v44 h-30 Z" />
            <path
              pathLength="1"
              d="M12 16 h18 M12 24 h16 M12 32 h18 M12 40 h10"
              style={{ strokeWidth: '2' }}
            />
          </svg>
          <div className="collage">
            <div className="card tilt-l reveal">
              <div className="blackhole">
                <span className="yell">HELLO??</span>
                <span style={{ marginTop: '6px' }}>
                  "thank you for your interest, we'll be in touch."
                </span>
                <span className="echo">(they will not be in touch)</span>
              </div>
              <span
                className="ct"
                style={{ textAlign: 'center', display: 'block', marginTop: '8px' }}
              >
                ↑ application status
              </span>
            </div>

            <div className="card tilt-r reveal">
              <span className="ct">the résumé robot</span>
              <div className="ats">
                <svg viewBox="0 0 80 80">
                  <rect
                    x="10"
                    y="20"
                    width="60"
                    height="46"
                    rx="10"
                    className="dood"
                    style={{ fill: '#2a1f1f', stroke: '#e24b4a' }}
                  />
                  <circle cx="30" cy="36" r="9" fill="#fff" stroke="#e24b4a" strokeWidth="2" />
                  <circle cx="52" cy="36" r="9" fill="#fff" stroke="#e24b4a" strokeWidth="2" />
                  <circle cx="32" cy="38" r="3" fill="#111" />
                  <circle cx="54" cy="34" r="3" fill="#111" />
                  <path
                    d="M24 54 q16 8 32 0"
                    fill="none"
                    stroke="#e24b4a"
                    strokeWidth="3"
                    strokeLinecap="round"
                  />
                  <path
                    d="M26 54 l4 6 m6 -6 l3 7 m6 -7 l4 6 m6 -6 l3 7"
                    stroke="#e24b4a"
                    strokeWidth="2"
                  />
                </svg>
                <span>
                  <b>BEEP. REJECTED.</b>
                  <br />a human will never see this.
                </span>
              </div>
            </div>

            <div className="card tilt-s reveal">
              <span className="ct">recruiters (every one of them)</span>
              <div className="npcs">
                <div className="npc">
                  <svg viewBox="0 0 40 46">
                    <circle cx="20" cy="14" r="10" fill="none" stroke="#8e80a4" strokeWidth="2.4" />
                    <path d="M8 44 q12 -16 24 0" fill="none" stroke="#8e80a4" strokeWidth="2.4" />
                    <path d="M14 14 h3 m6 0 h3" stroke="#8e80a4" strokeWidth="2" />
                    <path d="M15 19 q5 3 10 0" stroke="#8e80a4" strokeWidth="2" fill="none" />
                  </svg>
                  "on file"
                </div>
                <div className="npc">
                  <svg viewBox="0 0 40 46">
                    <circle cx="20" cy="14" r="10" fill="none" stroke="#8e80a4" strokeWidth="2.4" />
                    <path d="M8 44 q12 -16 24 0" fill="none" stroke="#8e80a4" strokeWidth="2.4" />
                    <path d="M14 14 h3 m6 0 h3" stroke="#8e80a4" strokeWidth="2" />
                    <path d="M15 19 q5 3 10 0" stroke="#8e80a4" strokeWidth="2" fill="none" />
                  </svg>
                  "more senior"
                </div>
                <div className="npc">
                  <svg viewBox="0 0 40 46">
                    <circle cx="20" cy="14" r="10" fill="none" stroke="#8e80a4" strokeWidth="2.4" />
                    <path d="M8 44 q12 -16 24 0" fill="none" stroke="#8e80a4" strokeWidth="2.4" />
                    <path d="M14 14 h3 m6 0 h3" stroke="#8e80a4" strokeWidth="2" />
                    <path d="M15 19 q5 3 10 0" stroke="#8e80a4" strokeWidth="2" fill="none" />
                  </svg>
                  "more junior"
                </div>
                <div className="npc">
                  <svg viewBox="0 0 40 46">
                    <circle cx="20" cy="14" r="10" fill="none" stroke="#8e80a4" strokeWidth="2.4" />
                    <path d="M8 44 q12 -16 24 0" fill="none" stroke="#8e80a4" strokeWidth="2.4" />
                    <path d="M14 14 h3 m6 0 h3" stroke="#8e80a4" strokeWidth="2" />
                    <path d="M15 19 q5 3 10 0" stroke="#8e80a4" strokeWidth="2" fill="none" />
                  </svg>
                  "someone"
                </div>
              </div>
            </div>

            <div className="card tilt-l reveal">
              <span className="ct">meanwhile, on LinkedIn</span>
              <div className="feed">
                <div className="scroller">
                  <p>🎉 thrilled to announce</p>
                  <p>humbled and honored</p>
                  <p>after 47 rounds of interviews, delighted to share…</p>
                  <p>so grateful for this journey</p>
                  <p>🎉 thrilled to announce</p>
                  <p>humbled and honored</p>
                  <p>after 47 rounds of interviews, delighted to share…</p>
                  <p>so grateful for this journey</p>
                </div>
              </div>
              <span className="ct" style={{ marginTop: '8px' }}>
                ghosted by 14 companies and one (1) person you actually liked.
              </span>
            </div>
          </div>

          <svg
            className="deco draw sm-hide"
            style={{ left: '8%', bottom: '24px', width: '36px' }}
            viewBox="0 0 44 60"
            aria-hidden="true"
          >
            <path pathLength="1" d="M22 4 q-9 10 0 18 q9 8 0 18 v10" />
            <path pathLength="1" d="M22 50 l-7 -8 M22 50 l7 -8" />
          </svg>
          <p className="counter reveal">
            applications: <span data-to="1000">0</span> · responses: 0 · dignity: offline
          </p>
        </div>
      </section>

      {/* ============ BEAT 3 — DEEP FRIED ============ */}
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
            it <b>HITS 24 boards</b> while you do NOTHING. it <b>writes the cover letter</b> — the
            one you CRIED over. GONE. it <b>scores your résumé</b> so the regex blob can't hurt you
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

      {/* ============ BEAT 4 — GODMODE ============ */}
      <section className="stage beat4">
        <div className="photo" />
        <div className="grade" />
        <div className="sun" />
        <svg
          className="deco draw sm-hide"
          style={{ left: '14%', top: '13%', width: '90px' }}
          viewBox="0 0 90 40"
          aria-hidden="true"
        >
          <path pathLength="1" d="M6 24 q9 -12 18 0 q9 -12 18 0" style={{ stroke: '#5a3610' }} />
          <path
            pathLength="1"
            d="M52 10 q7 -9 14 0 q7 -9 14 0"
            style={{ stroke: '#5a3610', strokeWidth: '2.4' }}
          />
        </svg>
        <svg
          className="deco draw sm-hide"
          style={{ left: '5%', top: '30%', width: '120px' }}
          viewBox="0 0 120 54"
          aria-hidden="true"
        >
          <path
            pathLength="1"
            d="M16 42 q-12 -2 -8 -12 q-2 -12 12 -12 q4 -12 18 -8 q10 -8 20 0 q14 -4 16 8 q12 2 8 14 q2 10 -12 10 H24"
            style={{ stroke: '#7a4a14', strokeWidth: '2.8' }}
          />
        </svg>
        <svg
          className="deco draw sm-hide sun-rays"
          style={{ right: 'calc(8% - 30px)', top: 'calc(10% - 30px)', width: '150px' }}
          viewBox="0 0 150 150"
          aria-hidden="true"
        >
          <path
            pathLength="1"
            d="M75 6 v14 M75 130 v14 M6 75 h14 M130 75 h14 M26 26 l10 10 M114 114 l10 10 M124 26 l-10 10 M36 114 l-10 10"
            style={{ stroke: '#b8770e', strokeWidth: '3' }}
          />
        </svg>
        <svg
          className="deco draw sm-hide"
          style={{ right: '14%', top: '38%', width: '90px' }}
          viewBox="0 0 110 50"
          aria-hidden="true"
        >
          <path
            pathLength="1"
            d="M16 38 q-10 -2 -6 -10 q-2 -10 10 -10 q4 -10 16 -6 q10 -6 18 0 q12 -2 12 8 q10 2 6 12 q2 8 -10 8 H22"
            style={{ stroke: '#7a4a14', strokeWidth: '2.4' }}
          />
        </svg>
        <svg
          className="deco draw sm-hide"
          style={{ left: '10%', bottom: '8%', width: '44px' }}
          viewBox="0 0 44 64"
          aria-hidden="true"
        >
          <path
            pathLength="1"
            d="M22 60 q2 -14 0 -26"
            style={{ stroke: '#2a6d2a', strokeWidth: '2.6' }}
          />
          <path
            pathLength="1"
            d="M22 40 q-10 6 -12 -2 M22 44 q10 4 12 -4"
            style={{ stroke: '#2a6d2a', strokeWidth: '2.2' }}
          />
          <path
            pathLength="1"
            d="M22 30 q-8 -2 -6 -8 q2 -6 8 -4 q2 -6 8 -2 q6 2 2 8 q4 6 -4 8 q-6 2 -8 -2 Z"
            style={{ stroke: '#a3541c', strokeWidth: '2.4' }}
          />
        </svg>
        <svg
          className="deco draw sm-hide"
          style={{ left: '34%', top: '8%', width: '54px' }}
          viewBox="0 0 60 26"
          aria-hidden="true"
        >
          <path
            pathLength="1"
            d="M4 16 q5 -7 10 0 q5 -7 10 0"
            style={{ stroke: '#5a3610', strokeWidth: '2' }}
          />
          <path
            pathLength="1"
            d="M36 8 q4 -6 8 0 q4 -6 8 0"
            style={{ stroke: '#5a3610', strokeWidth: '1.8' }}
          />
        </svg>
        <svg className="stonks" viewBox="0 0 100 70">
          <path
            d="M6 64 L34 40 L52 50 L92 8"
            fill="none"
            stroke="#2a7d2a"
            strokeWidth="5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <path
            d="M74 8 L92 8 L92 26"
            fill="none"
            stroke="#2a7d2a"
            strokeWidth="5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
        <div className="inner">
          <h2 className="sr-only">godmode</h2>
          <div
            className="doodle beat4-dood reveal tap"
            data-scream=""
            data-voice="smug"
            data-lines="effortless.|told you.|still got it.|too easy.|yawn."
            role="button"
            tabIndex={0}
            aria-label="poke the smug guy"
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
          <p className="big reveal">
            "i haven't opened LinkedIn in 3 weeks and i've never been happier."
          </p>
          <p className="counter reveal">
            matches found: <span data-to="247">0</span> · drafts written:{' '}
            <span data-to="247">0</span> · applications you actually sent:{' '}
            <span data-to="6">0</span>
          </p>
          <p className="line reveal">
            while you sleep, it hunts. while you cry, it writes. it does not cry. be like the
            autopilot.
          </p>
          <p className="line reveal">
            runs 100% offline with Ollama. your 4am desperation is between you, your laptop, and
            God. not OpenAI. <b>God.</b>
          </p>
        </div>
      </section>
    </>
  );
}
