import { BackLink } from '@/components/BackLink';

// <aside> of /how-it-works, split out of HowItWorksBody.tsx purely for file
// size — verbatim ported markup, no props. See HowItWorksBody.tsx for the
// shared conversion notes; public/scripts/how-it-works-0.js binds to #nav and
// '#nav button' by id/class (ADR 0018).
export function Sidebar() {
  return (
    <aside>
      <BackLink />
      <div className="brand">
        <div className="logo">A</div>
        <div>
          <h1>AI Job Hunter</h1>
          <p>Architecture walkthrough</p>
        </div>
      </div>
      <nav id="nav" aria-label="Page sections">
        <button data-view="overview" className="active" aria-current="true">
          <span className="nidx">01</span> Overview
        </button>
        <button data-view="boot" aria-current="false">
          <span className="nidx">02</span> Boot sequence
        </button>
        <button data-view="flows" aria-current="false">
          <span className="nidx">03</span> Flow simulator
        </button>
        <button data-view="ipc" aria-current="false">
          <span className="nidx">04</span> IPC reference
        </button>
        <button data-view="subsystems" aria-current="false">
          <span className="nidx">05</span> Subsystems
        </button>
        <button data-view="cheatsheet" aria-current="false">
          <span className="nidx">06</span> Interview cheat‑sheet
        </button>
      </nav>
      <div className="legend">
        <div>
          <span className="dot" style={{ background: 'var(--ui)' }} />
          Renderer (React)
        </div>
        <div>
          <span className="dot" style={{ background: 'var(--ipc)' }} />
          IPC bridge (AppClient)
        </div>
        <div>
          <span className="dot" style={{ background: 'var(--rust)' }} />
          Rust core (Tauri)
        </div>
        <div>
          <span className="dot" style={{ background: 'var(--ext)' }} />
          External services
        </div>
        <div style={{ marginTop: '10px', opacity: '0.7' }}>
          Static doc · no network · offline‑safe
        </div>
      </div>
    </aside>
  );
}
