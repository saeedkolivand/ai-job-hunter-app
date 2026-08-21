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
export function Growth() {
  return (
    <section className="growth">
      <div className="growth-charts">
        <img
          src="/stars.svg"
          width={860}
          height={360}
          alt="Line chart of the AI Job Hunter GitHub repository's star count climbing over time"
          loading="lazy"
        />
        <img
          src="/downloads.svg"
          width={860}
          height={360}
          alt="Line chart of AI Job Hunter's installer download count climbing over time"
          loading="lazy"
        />
      </div>
      <p className="growth-note">
        both redraw themselves every night — unlike my employment status.
      </p>
    </section>
  );
}
