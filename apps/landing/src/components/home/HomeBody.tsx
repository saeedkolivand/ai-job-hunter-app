import { CHROME_EXT, FIREFOX_EXT, GITHUB_REPO, KOFI, PAYPAL, SPONSOR } from '@/lib/site-links';

import { CookieGag } from './CookieGag';
import { HomeBeats } from './HomeBeats';

// Body markup for / (home), converted 1:1 from the deleted
// src/content/home/body.html. The rendered DOM must stay byte-identical —
// public/scripts/home-0.js (loader, scroll progress/odometer, the #journey SVG
// scrubbing, poke-a-doodle via [data-scream]/[data-voice]/[data-lines], the
// sound toggle, konami) binds to it by id/class/data-* (ADR 0018). beat1-4 are
// split into HomeBeats.tsx purely for file size — no props, same mechanical
// conversion. The root <div style={{display:'contents'}}> replaces the old
// RawHtml wrapper so the serialized DOM is unchanged. #cookie's only inline
// handler moved to CookieGag ('use client') — see scripts/diff-dom.mjs for why
// that's safe to diff around.
export function HomeBody() {
  return (
    <div style={{ display: 'contents' }}>
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

      <CookieGag />

      <div id="slowdown">slow down — you're making it worse</div>
      <div id="jk">just kidding.</div>

      <main>
        {/* ============ HERO ============ */}
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
            <a className="filmhint reveal" href="/download">
              already sold? → just take the app
            </a>
          </div>
        </section>

        <HomeBeats />

        {/* ============ FEATURES ============ */}
        <section className="stage features">
          <div className="inner">
            <svg
              className="deco draw sm-hide"
              style={{ left: '-10px', top: '-6px', width: '46px' }}
              viewBox="0 0 46 46"
              aria-hidden="true"
            >
              <path
                pathLength="1"
                d="M23 3 v12 M23 31 v12 M3 23 h12 M31 23 h12"
                style={{ strokeWidth: '2.6' }}
              />
              <path
                pathLength="1"
                d="M9 9 l8 8 M29 29 l8 8 M37 9 l-8 8 M17 29 l-8 8"
                style={{ strokeWidth: '2.2' }}
              />
            </svg>
            <svg
              className="deco draw sm-hide"
              style={{ right: '-8px', top: '4px', width: '44px', transform: 'rotate(8deg)' }}
              viewBox="0 0 46 60"
              aria-hidden="true"
            >
              <path
                pathLength="1"
                d="M23 4 a14 14 0 0 1 8 25 q-3 3 -3 7 H18 q0 -4 -3 -7 a14 14 0 0 0 8 -25 Z"
                style={{ strokeWidth: '2.6' }}
              />
              <path
                pathLength="1"
                d="M18 42 h10 M19 47 h8 M21 52 h4"
                style={{ strokeWidth: '2.2' }}
              />
            </svg>
            <svg
              className="deco draw sm-hide"
              style={{ left: '10%', top: '172px', width: '50px' }}
              viewBox="0 0 52 52"
              aria-hidden="true"
            >
              <path pathLength="1" d="M14 26 a12 12 0 1 0 24 0 a12 12 0 1 0 -24 0" />
              <path
                pathLength="1"
                d="M26 4 v7 M26 41 v7 M4 26 h7 M41 26 h7 M10 10 l5 5 M37 37 l5 5 M42 10 l-5 5 M15 37 l-5 5"
                style={{ strokeWidth: '2.6' }}
              />
              <path
                pathLength="1"
                d="M21 26 a5 5 0 1 0 10 0 a5 5 0 1 0 -10 0"
                style={{ strokeWidth: '2.2' }}
              />
            </svg>
            <svg
              className="deco draw sm-hide"
              style={{ right: '10%', top: '160px', width: '46px' }}
              viewBox="0 0 48 52"
              aria-hidden="true"
            >
              <path
                pathLength="1"
                d="M4 8 h16 v16 H4 Z M6 16 l5 5 9 -11"
                style={{ strokeWidth: '2.4' }}
              />
              <path
                pathLength="1"
                d="M4 32 h16 v16 H4 Z M6 40 l5 5 9 -11"
                style={{ strokeWidth: '2.4' }}
              />
              <path
                pathLength="1"
                d="M28 14 h16 M28 22 h12 M28 38 h16 M28 46 h12"
                style={{ strokeWidth: '2.2' }}
              />
            </svg>
            <h2 className="reveal">what it actually does</h2>
            <p className="lede reveal">(in between the breakdowns, real software happened)</p>
            <svg className="crayon-arrow" viewBox="0 0 120 60">
              <path
                d="M8 12 q40 36 90 30"
                fill="none"
                stroke="var(--ink)"
                strokeWidth="3"
                strokeLinecap="round"
              />
              <path
                d="M84 48 l14 -6 m-14 6 l8 -13"
                fill="none"
                stroke="var(--ink)"
                strokeWidth="3"
                strokeLinecap="round"
              />
            </svg>
            <div className="feat-grid">
              <div className="feat reveal">
                <h3>24 boards, one search</h3>
                <p>
                  LinkedIn direct; the walled boards (Indeed, Glassdoor, StepStone, Xing, Workday)
                  via aggregator API — no new Workday account. Then Greenhouse, Lever, Ashby, and
                  the whole DACH lineup — 15 more places to be rejected, but faster.
                </p>
              </div>
              <div className="feat reveal">
                <h3>AI cover letters &amp; résumés</h3>
                <p>
                  Writes it for you. 12 templates — 7 ATS-Safe tier (Classic, Swiss Minimal,
                  Academic, Meridian, Throughline, Cadence, Regent) and 5 Design tier (Atelier,
                  Portrait, Lebenslauf, Aria, Saffron), four with a headshot — rendered by a
                  pure-Rust Typst engine to DOCX, PDF &amp; TXT. The AI is more passionate about
                  this role than you could ever convincingly fake.
                </p>
              </div>
              <div className="feat reveal">
                <h3>ATS scoring</h3>
                <p>
                  Your résumé was reviewed by a piece of regex that has never felt joy. Now you
                  score back.
                </p>
              </div>
              <div className="feat reveal">
                <h3>Semantic matching</h3>
                <p>
                  Hybrid vector search. Understands your résumé better than your mother, who still
                  thinks you "do computers."
                </p>
              </div>
              <div className="feat reveal">
                <h3>Autopilot</h3>
                <p>
                  Pick a board and a schedule, walk away. At 9am it scrapes, ranks the matches,
                  pings you, and pre-writes each tailored application. The submit button it leaves
                  to you — some dignity must remain.
                </p>
              </div>
              <div className="feat reveal">
                <h3>Local, cloud, or CLI agents</h3>
                <p>
                  Run it free and offline with Ollama, drop in your own OpenAI/Anthropic/Gemini key,
                  or route it through a CLI agent you already pay for — Claude Code, Codex, or
                  Gemini CLI, no key needed. Your coding-agent subscription can finally do something
                  about the unemployment.
                </p>
              </div>
              <div className="feat reveal">
                <h3>Company research, on tap</h3>
                <p>
                  Before it writes the letter, it quietly looks the company up — provider-native web
                  search folded right into the prompt — so you don't have to fake admiring their
                  "mission." Opt-in. The robot does the admiring.
                </p>
              </div>
              <div className="feat reveal">
                <h3>Import anything (even a photo)</h3>
                <p>
                  Feed it your old résumé as a PDF, DOCX, or a cursed phone photo — Tesseract OCRs
                  it, the app re-parses it, and politely says nothing about the typos.
                </p>
              </div>
              <div className="feat reveal">
                <h3>11 languages (well, almost)</h3>
                <p>
                  It <i>writes</i> your résumé in 11 — en, de, fr, es, it, tr, pt, ru, zh, ja, ko.
                  The app's own interface speaks two for now, English and German; the other nine are
                  "coming soon," same as my big break. Cosmopolitan failure, localized.
                </p>
              </div>
              <div className="feat reveal">
                <h3>Privacy-first</h3>
                <p>
                  OS keychain, local SQLite, zero telemetry. No one is watching you spiral. For
                  once.
                </p>
              </div>
              <div className="feat reveal">
                <h3>Browser extension (save jobs one-click)</h3>
                <p>
                  While browsing any job board, one click imports that posting straight into your
                  desktop app — no parsing, no copy-paste. The extension does the grabbing so your
                  dignity doesn't have to.{' '}
                  <a
                    href={CHROME_EXT}
                    target="_blank"
                    rel="noopener noreferrer"
                    style={{ color: 'var(--red)' }}
                  >
                    Chrome
                  </a>{' '}
                  ·{' '}
                  <a
                    href={FIREFOX_EXT}
                    target="_blank"
                    rel="noopener noreferrer"
                    style={{ color: 'var(--red)' }}
                  >
                    Firefox
                  </a>
                  .
                </p>
              </div>
            </div>
          </div>
        </section>

        {/* ============ TESTIMONIALS ============ */}
        <section className="testi">
          <svg
            className="deco draw sm-hide"
            style={{ left: '6%', top: '34px', width: '54px' }}
            viewBox="0 0 60 44"
            aria-hidden="true"
          >
            <path
              pathLength="1"
              d="M24 6 Q10 10 10 24 q0 12 10 12 q8 0 8 -8 q0 -8 -8 -8 q1 -7 7 -10"
            />
            <path
              pathLength="1"
              d="M52 6 Q38 10 38 24 q0 12 10 12 q8 0 8 -8 q0 -8 -8 -8 q1 -7 7 -10"
            />
          </svg>
          <svg
            className="deco draw sm-hide"
            style={{ right: '6%', top: '34px', width: '54px', transform: 'rotate(180deg)' }}
            viewBox="0 0 60 44"
            aria-hidden="true"
          >
            <path
              pathLength="1"
              d="M24 6 Q10 10 10 24 q0 12 10 12 q8 0 8 -8 q0 -8 -8 -8 q1 -7 7 -10"
            />
            <path
              pathLength="1"
              d="M52 6 Q38 10 38 24 q0 12 10 12 q8 0 8 -8 q0 -8 -8 -8 q1 -7 7 -10"
            />
          </svg>
          <svg
            className="deco draw xl-only"
            style={{ left: '2%', top: '42%', width: '36px' }}
            viewBox="0 0 40 40"
            aria-hidden="true"
          >
            <path
              pathLength="1"
              d="M20 3 l4.5 11 12 1 -9 8 3 12 -10.5 -7 -10.5 7 3 -12 -9 -8 12 -1 Z"
              style={{ strokeWidth: '2.4' }}
            />
          </svg>
          <svg
            className="deco draw xl-only"
            style={{ right: '2%', top: '60%', width: '28px', transform: 'rotate(-12deg)' }}
            viewBox="0 0 40 40"
            aria-hidden="true"
          >
            <path
              pathLength="1"
              d="M20 3 l4.5 11 12 1 -9 8 3 12 -10.5 -7 -10.5 7 3 -12 -9 -8 12 -1 Z"
              style={{ strokeWidth: '2.2' }}
            />
          </svg>
          <svg
            className="deco draw xl-only"
            style={{ left: '3%', bottom: '70px', width: '46px' }}
            viewBox="0 0 50 50"
            aria-hidden="true"
          >
            <path pathLength="1" d="M8 26 h8 v18 H8 Z" style={{ strokeWidth: '2.4' }} />
            <path
              pathLength="1"
              d="M16 30 q1 -12 5 -17 q4 -5 6 1 q1 4 -2 9 h10 q5 0 4 5 l-3 12 q-1 4 -6 4 H16"
              style={{ strokeWidth: '2.4' }}
            />
          </svg>
          <h2>
            what the people are saying
            <br />
            <span className="small">(the people are not real)</span>
          </h2>
          <div className="wall">
            <div className="quote">
              <p>"I applied to 0 jobs and got 0 rejections. 10/10."</p>
              <span className="who">— a guy</span>
            </div>
            <div className="quote">
              <p>"haven't felt my hands in weeks. great app."</p>
              <span className="who">— power user</span>
            </div>
            <div className="quote">
              <p>
                "let it write every application. landed 6 interviews. ghosted all of them. we are
                the same now."
              </p>
              <span className="who">— finally, revenge</span>
            </div>
            <div className="quote">
              <p>"my therapist asked where the rage went. it's in the regex now."</p>
              <span className="who">— anonymous</span>
            </div>
            <div className="quote">
              <p>"downloaded it, my MacBook said the app was 'damaged.' relatable."</p>
              <span className="who">— early adopter</span>
            </div>
            <div className="quote">
              <p>"I don't have a job but I have a workflow."</p>
              <span className="who">— verified ✓ (not verified)</span>
            </div>
            <div className="quote">
              <p>
                "it hits Workday through an aggregator API so I never had to make a Workday account
                again. I wept — the good kind."
              </p>
              <span className="who">— survivor</span>
            </div>
            <div className="quote">
              <p>"5 stars. would dissociate again."</p>
              <span className="who">— a review I'll never get, this isn't on the App Store</span>
            </div>
            <div className="quote">
              <p className="stars">★★★★★</p>
              <span className="who">— my mom (still confused about what I do)</span>
            </div>
          </div>
          <p className="featured">
            as featured in: <b>your group chat</b> · <b>one (1) reddit comment</b> ·{' '}
            <b>my mom's facebook</b>
          </p>
        </section>

        {/* ============ FINALE ============ */}
        <section className="finale">
          <svg
            className="inkline draw"
            style={{ width: 'min(220px,60vw)', marginBottom: '26px' }}
            viewBox="0 0 220 24"
            aria-hidden="true"
          >
            <path
              pathLength="1"
              d="M6 12 q13 -11 27 0 t27 0 t27 0 t27 0 t27 0 t27 0 t27 0"
              style={{ strokeWidth: '2.8' }}
            />
          </svg>
          <svg
            className="deco draw sm-hide"
            style={{ left: '9%', top: '16%', width: '60px' }}
            viewBox="0 0 64 64"
            aria-hidden="true"
          >
            <path
              pathLength="1"
              d="M8 16 l8 6 M30 6 v8 M52 12 l-7 6 M14 38 l7 -4 M50 36 l-8 -3"
              style={{ strokeWidth: '2.6' }}
            />
            <path
              pathLength="1"
              d="M22 22 q4 4 0 8 M42 22 q-4 5 1 9"
              style={{ strokeWidth: '2.2' }}
            />
          </svg>
          <svg
            className="deco draw sm-hide"
            style={{ right: '9%', top: '20%', width: '56px', transform: 'scale(-1,1)' }}
            viewBox="0 0 64 64"
            aria-hidden="true"
          >
            <path
              pathLength="1"
              d="M8 16 l8 6 M30 6 v8 M52 12 l-7 6 M14 38 l7 -4 M50 36 l-8 -3"
              style={{ strokeWidth: '2.6' }}
            />
            <path
              pathLength="1"
              d="M22 22 q4 4 0 8 M42 22 q-4 5 1 9"
              style={{ strokeWidth: '2.2' }}
            />
          </svg>
          <svg
            className="deco draw sm-hide"
            style={{ left: '6%', top: '44%', width: '44px' }}
            viewBox="0 0 48 80"
            aria-hidden="true"
          >
            <path
              pathLength="1"
              d="M24 4 q16 0 16 18 q0 16 -16 18 q-16 -2 -16 -18 q0 -18 16 -18 Z"
              style={{ strokeWidth: '2.6' }}
            />
            <path pathLength="1" d="M21 40 h6 l-3 6 Z" style={{ strokeWidth: '2' }} />
            <path
              pathLength="1"
              className="dashed"
              strokeDasharray=".06 .05"
              d="M24 46 q-8 10 0 18 q8 8 0 14"
              style={{ strokeWidth: '2' }}
            />
          </svg>
          <svg
            className="deco draw sm-hide"
            style={{ right: '6%', top: '48%', width: '46px', transform: 'rotate(-20deg)' }}
            viewBox="0 0 52 60"
            aria-hidden="true"
          >
            <path pathLength="1" d="M10 50 L24 22 L40 38 Z" style={{ strokeWidth: '2.6' }} />
            <path
              pathLength="1"
              d="M28 14 l2 -10 M36 18 l8 -8 M42 28 l10 -2"
              style={{ strokeWidth: '2.4' }}
            />
            <path
              pathLength="1"
              d="M33 6 h.1 M48 14 h.1 M50 34 h.1"
              style={{ strokeWidth: '3.6' }}
            />
          </svg>
          <div
            className="doodle finale-dood tap"
            id="finale-dood"
            data-scream=""
            data-voice="smug"
            data-lines="we're still here?|ok, last one|…just go apply"
            role="button"
            tabIndex={0}
            aria-label="poke the finale guy"
          >
            <div className="bubble" />
            <svg viewBox="0 0 200 175">
              <circle className="dood" cx="100" cy="66" r="26" />
              <circle className="dood-fill" cx="90" cy="64" r="2.6" />
              <circle className="dood-fill" cx="110" cy="64" r="2.6" />
              <path className="dood" d="M88 78 q12 9 24 0" id="finale-mouth" />
              <path className="dood" d="M100 92 L100 138" />
              <path className="dood" d="M100 104 q-22 12 -30 34" />
              <path className="dood" d="M100 104 q22 12 30 34" />
              <path className="dood" d="M100 138 L82 172" />
              <path className="dood" d="M100 138 L118 172" />
            </svg>
          </div>
          <p className="honest">
            yes, this app is real. yes, it actually pulls from 24 boards — LinkedIn over HTTP, the
            walled ones (Indeed, Glassdoor, StepStone, Xing, Workday) via the Adzuna/JSearch
            aggregator API so no one has to make a 9th Workday account, and a bunch of ATS and DACH
            boards on top. yes, it's built with Tauri + Rust + React 19 + a vector database + a
            pure-Rust Typst engine that renders every PDF — because I had a lot of free time, on
            account of the unemployment. no, it does not auto-apply — it finds the jobs and writes
            the whole application; hitting submit is the one job left to you. no, I still don't have
            a job. the autopilot is doing its best.
          </p>
          <a className="cta" id="cta" href="/download">
            ok fine, take the app →
          </a>
          <a className="src-link" href={GITHUB_REPO}>
            view the source — it's PolyForm Noncommercial: read it, fork it, learn from it. just
            don't sell my misery back to me.
          </a>
          <a className="src-link" href="/creature">
            ▶ THE CREATURE — a hand-drawn doodle about the tiny recruiter you accidentally summon.
            it grows. (2:40)
          </a>
          <a className="src-link" href="/world">
            → or fly through the world (new)
          </a>
          <p className="src-link" style={{ marginTop: '24px', fontSize: '14px', color: '#6a614b' }}>
            or fund a man's job hunt →{' '}
            <a href={KOFI} target="_blank" rel="noopener" style={{ color: '#4a4233' }}>
              buy me a coffee
            </a>{' '}
            ·{' '}
            <a href={SPONSOR} target="_blank" rel="noopener" style={{ color: '#4a4233' }}>
              sponsor
            </a>{' '}
            ·{' '}
            <a href={PAYPAL} target="_blank" rel="noopener" style={{ color: '#4a4233' }}>
              PayPal
            </a>
          </p>
          <p className="footnote">
            macOS will say the app is "damaged." it's not damaged. it's just unsigned — like a
            contract I was never offered. run <code>xattr -cr</code> and we move on.
          </p>
          <p className="builtwith">
            Tauri · Rust · React 19 · TanStack · SQLite · Typst · Ollama · pure spite
          </p>
          <p className="byline">made by Saeed, between rejections.</p>
          <svg
            className="inkline draw"
            style={{ width: '24px', marginTop: '10px' }}
            viewBox="0 0 30 28"
            aria-hidden="true"
          >
            <path
              pathLength="1"
              d="M15 24 C5 16 2 9 8 5 q5 -3 7 4 q2 -7 7 -4 c6 4 3 11 -7 19 Z"
              style={{ strokeWidth: '2.4' }}
            />
          </svg>
          <p className="foot-nav">
            home · <a href="/download">download</a> · <a href="/privacy">privacy</a> ·{' '}
            <a href="/creature">▶ the short film</a> ·{' '}
            <a href={GITHUB_REPO} target="_blank" rel="noopener noreferrer">
              GitHub
            </a>{' '}
            ·{' '}
            <a href={CHROME_EXT} target="_blank" rel="noopener noreferrer">
              Chrome extension
            </a>{' '}
            ·{' '}
            <a href={FIREFOX_EXT} target="_blank" rel="noopener noreferrer">
              Firefox extension
            </a>{' '}
            ·{' '}
            <a href={SPONSOR} target="_blank" rel="noopener noreferrer">
              ♥ sponsor
            </a>
          </p>
        </section>
      </main>
    </div>
  );
}
