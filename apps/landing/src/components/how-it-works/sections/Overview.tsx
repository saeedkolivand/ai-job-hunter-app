// #view-overview <section> of /how-it-works, split out of HowItWorksBody.tsx
// purely for file size — verbatim ported markup, no props. See
// HowItWorksBody.tsx for the shared conversion notes; public/scripts/
// how-it-works-0.js binds to #nodeDetail and '[data-nodes] .node'/.node by
// id/class/data-attr (ADR 0018).
export function Overview() {
  return (
    <section className="view active" id="view-overview">
      <div className="eyebrow">The big picture</div>
      <h2 className="title">A local‑first desktop app in three layers</h2>
      <p className="lede">
        AI Job Hunter is a <b>Tauri</b> desktop app: a React UI runs in a webview, talks to a{' '}
        <b>Rust</b> core over Tauri{"'"}s IPC, and the Rust core does the real work — calling AI
        models, scraping job boards, ranking and tailoring matches, and storing everything{' '}
        <b>on your machine</b>. Click any box to see what it does and where it lives.
      </p>

      <div className="flowline">
        <span className="pill" style={{ borderColor: 'var(--ui)', color: 'var(--ui)' }}>
          User clicks in UI
        </span>
        <span className="arrow">→</span>
        <span className="pill" style={{ borderColor: 'var(--ipc)', color: 'var(--ipc)' }}>
          Service hook → AppClient
        </span>
        <span className="arrow">→</span>
        <span className="pill" style={{ borderColor: 'var(--ipc)', color: 'var(--ipc)' }}>
          invoke(&apos;command&apos;)
        </span>
        <span className="arrow">→</span>
        <span className="pill" style={{ borderColor: 'var(--rust)', color: 'var(--rust)' }}>
          #[tauri::command]
        </span>
        <span className="arrow">→</span>
        <span className="pill" style={{ borderColor: 'var(--ext)', color: 'var(--ext)' }}>
          AI / board / disk
        </span>
        <span className="arrow">↺</span>
        <span className="pill" style={{ borderColor: 'var(--ipc)', color: 'var(--ipc)' }}>
          emit(&apos;event&apos;) stream
        </span>
        <span className="arrow">→</span>
        <span className="pill" style={{ borderColor: 'var(--ui)', color: 'var(--ui)' }}>
          UI updates live
        </span>
      </div>

      <div className="map" style={{ marginTop: '22px' }}>
        <div className="layer L-ui">
          <h4>
            <span className="dot" style={{ background: 'var(--ui)' }} />
            Renderer — React + TanStack Router
          </h4>
          <div className="nodes" data-nodes="">
            <div className="node" data-node="main">
              main.tsx (bootstrap)
            </div>
            <div className="node" data-node="routes">
              Routes / Pages
            </div>
            <div className="node" data-node="features">
              Features &amp; components
            </div>
            <div className="node" data-node="hooks">
              Service hooks (React Query)
            </div>
            <div className="node" data-node="stores">
              Zustand stores
            </div>
            <div className="node" data-node="machines">
              State machines
            </div>
          </div>
        </div>
        <div className="layer L-ipc">
          <h4>
            <span className="dot" style={{ background: 'var(--ipc)' }} />
            IPC bridge — AppClient
          </h4>
          <div className="nodes" data-nodes="">
            <div className="node" data-node="appclient">
              createTauriInvokeClient()
            </div>
            <div className="node" data-node="invoke">
              invoke(&apos;cmd&apos;, args)
            </div>
            <div className="node" data-node="listen">
              listen(&apos;channel&apos;) events
            </div>
            <div className="node" data-node="contracts">
              Contracts + Zod (shared)
            </div>
          </div>
        </div>
        <div className="layer L-rust">
          <h4>
            <span className="dot" style={{ background: 'var(--rust)' }} />
            Rust core — Tauri
          </h4>
          <div className="nodes" data-nodes="">
            <div className="node" data-node="commands">
              #[tauri::command] handlers
            </div>
            <div className="node" data-node="provider">
              AI provider layer
            </div>
            <div className="node" data-node="scraper">
              Scraper engine + boards
            </div>
            <div className="node" data-node="autopilot">
              Autopilot engine
            </div>
            <div className="node" data-node="jobtracker">
              JobTracker
            </div>
            <div className="node" data-node="pipeline">
              Pipeline (cover letter)
            </div>
            <div className="node" data-node="stores-rs">
              Stores (SQLite/JSON/keyring)
            </div>
          </div>
        </div>
        <div className="layer L-ext">
          <h4>
            <span className="dot" style={{ background: 'var(--ext)' }} />
            External services
          </h4>
          <div className="nodes" data-nodes="">
            <div className="node" data-node="ollama">
              Ollama (local LLM)
            </div>
            <div className="node" data-node="cloud">
              Cloud APIs
            </div>
            <div className="node" data-node="cli">
              CLI agents
            </div>
            <div className="node" data-node="boards">
              Job boards
            </div>
            <div className="node" data-node="disk">
              Disk &amp; OS keyring
            </div>
          </div>
        </div>
      </div>

      <div className="detail" id="nodeDetail" aria-live="polite" aria-atomic="true">
        <h4>Click a box above</h4>
        <p className="muted">
          Each box shows its responsibility, the real files it maps to, and the key types or
          functions an interviewer might ask about.
        </p>
      </div>
    </section>
  );
}
