// #view-ipc <section> of /how-it-works, split out of HowItWorksBody.tsx
// purely for file size — verbatim ported markup, no props. See
// HowItWorksBody.tsx for the shared conversion notes; public/scripts/
// how-it-works-0.js binds to #ipcFilter/#ipcCount/#ipcBody (ADR 0018).
export function IpcReference() {
  return (
    <section className="view" id="view-ipc">
      <div className="eyebrow">Every door between the two worlds</div>
      <h2 className="title">IPC reference</h2>
      <p className="lede">
        Every method on <code>AppClient</code>, the exact Tauri command it calls (or event channel
        it subscribes to), and the namespace it belongs to. This is the complete contract surface
        between the renderer and Rust. Type to filter.
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
        Source of truth: <span className="path">apps/desktop/src/tauri-client/namespaces/*</span>,
        assembled in <span className="path">apps/desktop/src/tauri-client/index.ts</span>. Request
        structs are generated from Zod schemas via <code>pnpm gen:ipc</code> into{' '}
        <span className="path">apps/desktop/src-tauri/src/ipc_contracts/</span>.
      </p>
    </section>
  );
}
