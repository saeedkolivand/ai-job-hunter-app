import { finale as f } from "@/content/finale";

// FINALE section, transcribed 1:1 from landing/index.html. Server component:
// the finale-dood is inert here; legacy.ts binds its scream via data-* attrs.
export default function Finale() {
  return (
    <section className="finale">
      <svg
        className="inkline draw"
        style={{ width: "min(220px,60vw)", marginBottom: "26px" }}
        viewBox="0 0 220 24"
        aria-hidden="true"
      >
        <path
          pathLength="1"
          d="M6 12 q13 -11 27 0 t27 0 t27 0 t27 0 t27 0 t27 0 t27 0"
          style={{ strokeWidth: "2.8" }}
        />
      </svg>
      <svg
        className="deco draw sm-hide"
        style={{ left: "9%", top: "16%", width: "60px" }}
        viewBox="0 0 64 64"
        aria-hidden="true"
      >
        <path
          pathLength="1"
          d="M8 16 l8 6 M30 6 v8 M52 12 l-7 6 M14 38 l7 -4 M50 36 l-8 -3"
          style={{ strokeWidth: "2.6" }}
        />
        <path
          pathLength="1"
          d="M22 22 q4 4 0 8 M42 22 q-4 5 1 9"
          style={{ strokeWidth: "2.2" }}
        />
      </svg>
      <svg
        className="deco draw sm-hide"
        style={{ right: "9%", top: "20%", width: "56px", transform: "scale(-1,1)" }}
        viewBox="0 0 64 64"
        aria-hidden="true"
      >
        <path
          pathLength="1"
          d="M8 16 l8 6 M30 6 v8 M52 12 l-7 6 M14 38 l7 -4 M50 36 l-8 -3"
          style={{ strokeWidth: "2.6" }}
        />
        <path
          pathLength="1"
          d="M22 22 q4 4 0 8 M42 22 q-4 5 1 9"
          style={{ strokeWidth: "2.2" }}
        />
      </svg>
      <svg
        className="deco draw sm-hide"
        style={{ left: "6%", top: "44%", width: "44px" }}
        viewBox="0 0 48 80"
        aria-hidden="true"
      >
        <path
          pathLength="1"
          d="M24 4 q16 0 16 18 q0 16 -16 18 q-16 -2 -16 -18 q0 -18 16 -18 Z"
          style={{ strokeWidth: "2.6" }}
        />
        <path
          pathLength="1"
          d="M21 40 h6 l-3 6 Z"
          style={{ strokeWidth: "2" }}
        />
        <path
          pathLength="1"
          className="dashed"
          strokeDasharray=".06 .05"
          d="M24 46 q-8 10 0 18 q8 8 0 14"
          style={{ strokeWidth: "2" }}
        />
      </svg>
      <svg
        className="deco draw sm-hide"
        style={{ right: "6%", top: "48%", width: "46px", transform: "rotate(-20deg)" }}
        viewBox="0 0 52 60"
        aria-hidden="true"
      >
        <path
          pathLength="1"
          d="M10 50 L24 22 L40 38 Z"
          style={{ strokeWidth: "2.6" }}
        />
        <path
          pathLength="1"
          d="M28 14 l2 -10 M36 18 l8 -8 M42 28 l10 -2"
          style={{ strokeWidth: "2.4" }}
        />
        <path
          pathLength="1"
          d="M33 6 h.1 M48 14 h.1 M50 34 h.1"
          style={{ strokeWidth: "3.6" }}
        />
      </svg>
      <div
        className="doodle finale-dood tap"
        id="finale-dood"
        data-scream=""
        data-voice={f.screamVoice}
        data-lines={f.screamLines}
        role="button"
        tabIndex={0}
        aria-label={f.doodleAria}
      >
        <div className="bubble"></div>
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
      <p className="honest">{f.honest}</p>
      <a className="cta" id="cta" href="/download">
        {f.cta}
      </a>
      <a
        className="src-link"
        href="https://github.com/saeedkolivand/ai-job-hunter-app"
      >
        {f.srcGithub}
      </a>
      <a className="src-link" href="/creature">
        {f.srcCreature}
      </a>
      <p
        className="src-link"
        style={{ marginTop: "24px", fontSize: "14px", color: "#6a614b" }}
      >
        {f.fundPrefix}
        <a
          href="https://ko-fi.com/saeedkolivand"
          target="_blank"
          rel="noopener"
          style={{ color: "#4a4233" }}
        >
          {f.fundCoffee}
        </a>
        {f.fundSep}
        <a
          href="https://github.com/sponsors/saeedkolivand"
          target="_blank"
          rel="noopener"
          style={{ color: "#4a4233" }}
        >
          {f.fundSponsor}
        </a>
        {f.fundSep}
        <a
          href="https://paypal.me/saeedkolivand"
          target="_blank"
          rel="noopener"
          style={{ color: "#4a4233" }}
        >
          {f.fundPaypal}
        </a>
      </p>
      <p className="footnote">
        {f.footnotePre}
        <code>{f.footnoteCode}</code>
        {f.footnotePost}
      </p>
      <p className="builtwith">{f.builtwith}</p>
      <p className="byline">{f.byline}</p>
      <svg
        className="inkline draw"
        style={{ width: "24px", marginTop: "10px" }}
        viewBox="0 0 30 28"
        aria-hidden="true"
      >
        <path
          pathLength="1"
          d="M15 24 C5 16 2 9 8 5 q5 -3 7 4 q2 -7 7 -4 c6 4 3 11 -7 19 Z"
          style={{ strokeWidth: "2.4" }}
        />
      </svg>
      <p className="foot-nav">
        {f.footHome}
        {f.footSep}
        <a href="/download">{f.footDownload}</a>
        {f.footSep}
        <a href="/privacy">{f.footPrivacy}</a>
        {f.footSep}
        <a href="/creature">{f.footFilm}</a>
        {f.footSep}
        <a
          href="https://github.com/saeedkolivand/ai-job-hunter-app"
          target="_blank"
          rel="noopener noreferrer"
        >
          {f.footGithub}
        </a>
        {f.footSep}
        <a
          href="https://chromewebstore.google.com/detail/ai-job-hunter-%E2%80%94-job-impor/oaoekkgkhmgdfnpmfkpphgiikliaicll"
          target="_blank"
          rel="noopener noreferrer"
        >
          {f.footChrome}
        </a>
        {f.footSep}
        <a
          href="https://addons.mozilla.org/en-US/firefox/addon/ai-job-hunter-job-importer/"
          target="_blank"
          rel="noopener noreferrer"
        >
          {f.footFirefox}
        </a>
        {f.footSep}
        <a
          href="https://github.com/sponsors/saeedkolivand"
          target="_blank"
          rel="noopener noreferrer"
        >
          {f.footSponsor}
        </a>
      </p>
    </section>
  );
}
