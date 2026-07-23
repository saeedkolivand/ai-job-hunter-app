import { BackLink } from '@/components/BackLink';
import { SiteFooter } from '@/components/SiteFooter';

// Body markup for /how-it-works, converted 1:1 from the deleted
// src/content/how-it-works/body.html. The rendered DOM must stay
// byte-identical — public/scripts/how-it-works-0.js (the sidebar tab
// switcher + boot/flow players + IPC filter) and how-it-works-1.js (the
// console egg) both bind to it by id/class/data-attr (ADR 0018). The root
// <div style={{display:'contents'}}> replaces the old <RawHtml> wrapper so
// the serialized DOM — including the CSS's `body footer` descendant
// selector — stays identical to the baseline.
export function HowItWorksBody() {
  return (
    <div style={{ display: 'contents' }}>
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

      <main>
        {/* ================= OVERVIEW ================= */}
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

        {/* ================= BOOT ================= */}
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

        {/* ================= FLOWS ================= */}
        <section className="view" id="view-flows">
          <div className="eyebrow">Click‑by‑click</div>
          <h2 className="title">Flow simulator — UI → IPC → Rust → result</h2>
          <p className="lede">
            Pick a journey and step through every hop. The strip at the top lights up the layer that
            is currently doing the work, so you can see exactly where control sits at each moment.
          </p>
          <div className="tabs" id="flowTabs"></div>
          <div id="flowPlayer"></div>
          <div className="note" id="autopilotNote">
            <b>Autopilot ties them together.</b> A background scheduler ticks every <b>60s</b> (
            <span className="path">autopilot_scheduler.rs</span>). For each due autopilot it runs
            the whole loop: <b>scrape</b> → <b>rank</b> (embed each posting, cosine‑compare to your
            resume) → <b>generate</b> a cover letter through the validated pipeline → <b>apply</b> →{' '}
            <b>record</b> the run. Every sub‑step reuses the exact same commands you stepped through
            above.
          </div>
        </section>

        {/* ================= IPC REFERENCE ================= */}
        <section className="view" id="view-ipc">
          <div className="eyebrow">Every door between the two worlds</div>
          <h2 className="title">IPC reference</h2>
          <p className="lede">
            Every method on <code>AppClient</code>, the exact Tauri command it calls (or event
            channel it subscribes to), and the namespace it belongs to. This is the complete
            contract surface between the renderer and Rust. Type to filter.
          </p>
          <div className="filterbar">
            <input
              id="ipcFilter"
              placeholder="Filter by namespace, method, or command…"
              aria-label="Filter IPC endpoints by namespace, method, or command"
            />
            <span className="count" id="ipcCount" aria-live="polite" aria-atomic="true"></span>
          </div>
          <div className="card" style={{ padding: '0', overflow: 'hidden' }}>
            <table aria-label="IPC endpoint reference">
              <caption className="sr-only">
                AppClient IPC endpoint reference — all invoke commands and event channels
              </caption>
              <thead>
                <tr>
                  <th>Namespace</th>
                  <th>Method</th>
                  <th>Kind</th>
                  <th>Tauri command / event channel</th>
                </tr>
              </thead>
              <tbody id="ipcBody"></tbody>
            </table>
          </div>
          <p className="footer">
            Source of truth:{' '}
            <span className="path">apps/desktop/src/tauri-client/namespaces/*</span>, assembled in{' '}
            <span className="path">apps/desktop/src/tauri-client/index.ts</span>. Request structs
            are generated from Zod schemas via <code>pnpm gen:ipc</code> into{' '}
            <span className="path">apps/desktop/src-tauri/src/ipc_contracts/</span>.
          </p>
        </section>

        {/* ================= SUBSYSTEMS ================= */}
        <section className="view" id="view-subsystems">
          <div className="eyebrow">Under the hood</div>
          <h2 className="title">Subsystem deep‑dives</h2>
          <p className="lede">
            The Rust core is built around a few registry‑driven subsystems. Each one is designed so
            a new provider or board is <b>one new module + one registry line</b> — no edits anywhere
            else.
          </p>
          <div id="subs"></div>
        </section>

        {/* ================= CHEAT SHEET ================= */}
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
              Claude Code), a scraping engine driven by a board registry, and an autopilot engine
              that scrapes, ranks and notifies you about new matches. Long jobs return a{' '}
              <code>jobId</code> immediately and stream progress back as Tauri events, so the UI
              stays live. Data — resumes, embeddings, history — lives in local SQLite/JSON, and
              secrets go in the OS keyring. Adding a new model provider or job board is a single
              module plus one registry entry.”
            </p>
          </div>
          <p className="footer">
            Generated as living documentation for <span className="path">apps/desktop</span>. Open
            this file directly in any browser — it needs no server.
          </p>
        </section>
      </main>
      <SiteFooter />
    </div>
  );
}
