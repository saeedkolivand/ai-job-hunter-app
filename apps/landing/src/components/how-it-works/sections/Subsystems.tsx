// #view-subsystems <section> of /how-it-works, split out of
// HowItWorksBody.tsx purely for file size — verbatim ported markup, no
// props. See HowItWorksBody.tsx for the shared conversion notes;
// public/scripts/how-it-works-0.js mounts the subsystem accordions into
// #subs (ADR 0018).
export function Subsystems() {
  return (
    <section className="view" id="view-subsystems">
      <div className="eyebrow">Under the hood</div>
      <h2 className="title">Subsystem deep‑dives</h2>
      <p className="lede">
        The Rust core is built around a few registry‑driven subsystems. Each one is designed so a
        new provider or board is <b>one new module + one registry line</b> — no edits anywhere else.
      </p>
      <div id="subs"></div>
    </section>
  );
}
