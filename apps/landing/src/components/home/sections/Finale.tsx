import { CHROME_EXT, FIREFOX_EXT, GITHUB_REPO, KOFI, PAYPAL, SPONSOR } from '@/lib/site-links';

// Finale <section> of / (home), split out of HomeBody.tsx purely for file
// size — verbatim ported markup, no props. See HomeBody.tsx for the shared
// conversion notes; public/scripts/home-0.js binds to #finale-dood, .finale,
// [data-scream]/[data-voice]/[data-lines] and #cta by id/class/data-*
// (ADR-0018).
export function Finale() {
  return (
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
        <path pathLength="1" d="M22 22 q4 4 0 8 M42 22 q-4 5 1 9" style={{ strokeWidth: '2.2' }} />
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
        <path pathLength="1" d="M22 22 q4 4 0 8 M42 22 q-4 5 1 9" style={{ strokeWidth: '2.2' }} />
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
        <path pathLength="1" d="M33 6 h.1 M48 14 h.1 M50 34 h.1" style={{ strokeWidth: '3.6' }} />
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
        walled ones (Indeed, Glassdoor, StepStone, Xing, Workday) via the Adzuna/JSearch aggregator
        API so no one has to make a 9th Workday account, and a bunch of ATS and DACH boards on top.
        yes, it's built with Tauri + Rust + React 19 + a vector database + a pure-Rust Typst engine
        that renders every PDF — because I had a lot of free time, on account of the unemployment.
        no, it does not auto-apply — it finds the jobs and writes the whole application; hitting
        submit is the one job left to you. no, I still don't have a job. the autopilot is doing its
        best.
      </p>
      <a className="cta" id="cta" href="/download">
        ok fine, take the app →
      </a>
      <a className="src-link" href={GITHUB_REPO}>
        view the source — it's PolyForm Noncommercial: read it, fork it, learn from it. just don't
        sell my misery back to me.
      </a>
      <a className="src-link" href="/creature">
        ▶ THE CREATURE — a hand-drawn doodle about the tiny recruiter you accidentally summon. it
        grows. (2:40)
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
        macOS will say the app is "damaged." it's not damaged. it's just unsigned — like a contract
        I was never offered. run <code>xattr -cr</code> and we move on.
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
        home · <a href="/how-it-works">how it works</a> · <a href="/download">download</a> ·{' '}
        <a href="/privacy">privacy</a> · <a href="/accessibility">accessibility</a> ·{' '}
        <a href="/creature">▶ the short film</a> · <a href="/agent-system">the agent fleet</a> ·{' '}
        <a href="/architecture-map">architecture</a> · <a href="/tech-radar">tech radar</a> ·{' '}
        <a href="/storybook/">design system</a> ·{' '}
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
  );
}
