import { CHROME_EXT, FIREFOX_EXT } from '@/lib/site-links';

// Features <section> of / (home), split out of HomeBody.tsx purely for
// file size — verbatim ported markup, no props. See HomeBody.tsx for the
// shared conversion notes; public/scripts/home-0.js binds to .stage/.reveal
// and .feat-grid by class (ADR-0018).
export function Features() {
  return (
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
          <path pathLength="1" d="M18 42 h10 M19 47 h8 M21 52 h4" style={{ strokeWidth: '2.2' }} />
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
              LinkedIn direct; the walled boards (Indeed, Glassdoor, StepStone, Xing, Workday) via
              aggregator API — no new Workday account. Then Greenhouse, Lever, Ashby, and the whole
              DACH lineup — 15 more places to be rejected, but faster.
            </p>
          </div>
          <div className="feat reveal">
            <h3>AI cover letters &amp; résumés</h3>
            <p>
              Writes it for you. 12 templates — 7 ATS-Safe tier (Classic, Swiss Minimal, Academic,
              Meridian, Throughline, Cadence, Regent) and 5 Design tier (Atelier, Portrait,
              Lebenslauf, Aria, Saffron), four with a headshot — rendered by a pure-Rust Typst
              engine to DOCX, PDF &amp; TXT. The AI is more passionate about this role than you
              could ever convincingly fake.
            </p>
          </div>
          <div className="feat reveal">
            <h3>ATS scoring</h3>
            <p>
              Your résumé was reviewed by a piece of regex that has never felt joy. Now you score
              back.
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
              Pick a board and a schedule, walk away. At 9am it scrapes, ranks the matches, pings
              you, and pre-writes each tailored application. The submit button it leaves to you —
              some dignity must remain.
            </p>
          </div>
          <div className="feat reveal">
            <h3>Local, cloud, or CLI agents</h3>
            <p>
              Run it free and offline with Ollama, drop in your own OpenAI/Anthropic/Gemini key, or
              route it through a CLI agent you already pay for — Claude Code, Codex, or Gemini CLI,
              no key needed. Your coding-agent subscription can finally do something about the
              unemployment.
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
              Feed it your old résumé as a PDF, DOCX, or a cursed phone photo — Tesseract OCRs it,
              the app re-parses it, and politely says nothing about the typos.
            </p>
          </div>
          <div className="feat reveal">
            <h3>11 languages (well, almost)</h3>
            <p>
              It <i>writes</i> your résumé in 11 — en, de, fr, es, it, tr, pt, ru, zh, ja, ko. The
              app's own interface speaks two for now, English and German; the other nine are "coming
              soon," same as my big break. Cosmopolitan failure, localized.
            </p>
          </div>
          <div className="feat reveal">
            <h3>Privacy-first</h3>
            <p>
              OS keychain, local SQLite, no analytics — just an opt-out crash report when it breaks.
              No one is watching you spiral. For once.
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
  );
}
