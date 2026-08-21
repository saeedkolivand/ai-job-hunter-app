// Growth <section> of / (home) — self-updating GitHub star/download charts,
// added after the original 8-section 1:1 port (see HomeBody.tsx for that
// history and the ADR-0018 DOM-fidelity note). The two SVGs are regenerated
// daily by .github/workflows/repo-charts.yml (scripts/build-repo-charts.mjs)
// and copied into apps/landing/public/ at build time by pages.yml, so they
// resolve same-origin at runtime (/stars.svg, /downloads.svg) — never
// fetched from a third party. Each SVG already carries its own title, value
// and date range internally, so no caption repeats that here. No id/class
// below is bound by public/scripts/home-0.js; the #journey line overlay
// picks this section up generically (it walks every `main > section` until
// it hits .finale, with no per-section special-casing).
//
// The alt text names what each chart PLOTS, never which way the line is
// going: the SVGs are rebuilt nightly from live data, so "climbing" is a
// claim about a number this file cannot see and would become a lie the first
// flat or falling week. It also stops short of the current totals — those
// live only inside the nightly SVG, which is gitignored and absent from a
// local checkout, so reading them at build time would break `next build`
// and the jsdom tests everywhere except CI. renderLineChart already emits
// role="img" + aria-label carrying the value and date; wiring that through
// to the page needs the numbers plumbed to build time as their own artifact.
export function Growth() {
  return (
    <section className="growth">
      <div className="growth-charts">
        <img
          src="/stars.svg"
          width={860}
          height={360}
          alt="Line chart of the AI Job Hunter GitHub repository's star count over time"
          loading="lazy"
        />
        <img
          src="/downloads.svg"
          width={860}
          height={360}
          alt="Line chart of AI Job Hunter's cumulative installer downloads over time"
          loading="lazy"
        />
      </div>
      <p className="growth-note">
        both redraw themselves every night — unlike my employment status.
      </p>
    </section>
  );
}
