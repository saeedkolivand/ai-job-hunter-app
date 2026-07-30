// #view-boot <section> of /how-it-works, split out of HowItWorksBody.tsx
// purely for file size — verbatim ported markup, no props. See
// HowItWorksBody.tsx for the shared conversion notes; public/scripts/
// how-it-works-0.js mounts the boot step player into #bootPlayer (ADR 0018).
export function Boot() {
  return (
    <section className="view" id="view-boot">
      <div className="eyebrow">What happens at launch</div>
      <h2 className="title">From double‑click to a ready window</h2>
      <p className="lede">
        Two things start in parallel: the <b>webview</b> mounts the React provider tree, and the{' '}
        <b>
          Rust <code>setup()</code>
        </b>{' '}
        wires up all shared state and background tasks. Step through it.
      </p>
      <div id="bootPlayer"></div>
    </section>
  );
}
