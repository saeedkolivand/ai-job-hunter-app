// #view-cheatsheet <section> of /how-it-works, split out of
// HowItWorksBody.tsx purely for file size — verbatim ported markup, no
// props. See HowItWorksBody.tsx for the shared conversion notes;
// public/scripts/how-it-works-0.js mounts the cheat-sheet accordions into
// #qa (ADR 0018).
export function CheatSheet() {
  return (
    <section className="view" id="view-cheatsheet">
      <div className="eyebrow">Be ready for anything</div>
      <h2 className="title">Interview cheat‑sheet</h2>
      <p className="lede">
        Tight, codebase‑grounded answers to the questions most likely to come up. Each cites the
        file you can point to.
      </p>
      <div id="qa"></div>
      <hr className="sep" />
      <div className="card">
        <h3 style={{ marginTop: '0' }}>30‑second elevator pitch</h3>
        <p className="muted">
          “It&apos;s a <b>local‑first Tauri desktop app</b> that automates job hunting. A
          React/TanStack renderer talks to a Rust core over Tauri IPC. The Rust side has a
          strictly‑typed AI‑provider layer (Ollama, the cloud APIs, and headless CLI agents like
          Claude Code), a scraping engine driven by a board registry, and an autopilot engine that
          scrapes, ranks and notifies you about new matches. Long jobs return a <code>jobId</code>{' '}
          immediately and stream progress back as Tauri events, so the UI stays live. Data —
          resumes, embeddings, history — lives in local SQLite/JSON, and secrets go in the OS
          keyring. Adding a new model provider or job board is a single module plus one registry
          entry.”
        </p>
      </div>
      <p className="footer">
        Generated as living documentation for <span className="path">apps/desktop</span>. Open this
        file directly in any browser — it needs no server.
      </p>
    </section>
  );
}
