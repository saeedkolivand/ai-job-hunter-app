// #view-flows <section> of /how-it-works, split out of HowItWorksBody.tsx
// purely for file size — verbatim ported markup, no props. See
// HowItWorksBody.tsx for the shared conversion notes; public/scripts/
// how-it-works-0.js mounts the flow tabs/player into #flowTabs/#flowPlayer
// (ADR 0018). #autopilotNote's prose is static.
export function Flows() {
  return (
    <section className="view" id="view-flows">
      <div className="eyebrow">Click‑by‑click</div>
      <h2 className="title">Flow simulator — UI → IPC → Rust → result</h2>
      <p className="lede">
        Pick a journey and step through every hop. The strip at the top lights up the layer that is
        currently doing the work, so you can see exactly where control sits at each moment.
      </p>
      <div className="tabs" id="flowTabs"></div>
      <div id="flowPlayer"></div>
      <div className="note" id="autopilotNote">
        <b>Autopilot ties them together.</b> A background scheduler ticks every <b>60s</b> (
        <span className="path">autopilot_scheduler.rs</span>). For each due autopilot it runs the
        whole loop: <b>scrape</b> → <b>rank</b> (embed each posting, cosine‑compare to your resume)
        → <b>generate</b> a cover letter through the validated pipeline → <b>apply</b> →{' '}
        <b>record</b> the run. Every sub‑step reuses the exact same commands you stepped through
        above.
      </div>
    </section>
  );
}
