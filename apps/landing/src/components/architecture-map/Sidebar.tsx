import type { ReactNode } from 'react';

import {
  clusters,
  edges,
  FINDINGS,
  FIXES,
  KNOWN_BUGS,
  type MapNode,
  nodes,
} from '@/data/architecture-map';

const byId = new Map(nodes.map((n) => [n.id, n]));

// ── sidebar (React state — low-frequency: changes on hover/click/clear) ───────
export type Panel = { kind: 'default' } | { kind: 'node'; id: string };

export function nodeAria(n: MapNode): string {
  return `${n.label}${n.sub ? ' — ' + n.sub : ''}${n.role ? '. ' + n.role : ''}`;
}

// Render verbatim findings prose, turning its <b>…</b> emphasis into real React
// elements — never innerHTML / dangerouslySetInnerHTML (ADR-0018 origin invariant).
function renderRich(text: string): ReactNode[] {
  const out: ReactNode[] = [];
  const re = /<b>(.*?)<\/b>/g;
  let last = 0;
  let key = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) out.push(text.slice(last, m.index));
    out.push(<b key={key++}>{m[1]}</b>);
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

// Every sidebar section is a `.k` heading + a body — collapsed to one shape
// since all 9 call sites share it exactly (some just wrap a conditional).
function Field({ heading, children }: { heading: ReactNode; children: ReactNode }) {
  return (
    <>
      <div className="k">{heading}</div>
      {children}
    </>
  );
}

function DefaultSidebar() {
  return (
    <>
      <h2>AI Job Hunter — architecture</h2>
      <div className="role">
        Local-first Tauri 2 desktop app. The renderer talks to a Rust core only through typed IPC
        contracts (ports &amp; adapters). Heavy work — scraping, document extraction, AI generation,
        embeddings — runs natively in-process.
      </div>
      <div className="plain">
        Click any box for its plain-English role, real file path, and what wires into and out of it.
        Use the chips up top to highlight a feature or a user-flow path. Drag to pan, scroll to
        zoom.
      </div>
      <Field heading="Notable findings from this map">
        <ul className="findings">
          {FINDINGS.map((f, i) => (
            <li key={i}>{renderRich(f)}</li>
          ))}
        </ul>
      </Field>
      <Field heading="Counts">
        <div style={{ color: 'var(--faint)' }}>
          {nodes.length} nodes · {edges.length} edges · {clusters.length} clusters · 24 boards · 8
          AI providers
        </div>
      </Field>
      <Field heading="Critical paths">
        <div style={{ color: 'var(--faint)' }}>
          Red = <b>AI Generate</b> (default). Click <b>Autopilot</b> or <b>Scrape → Match</b> to
          light up the other two flows.
        </div>
      </Field>
    </>
  );
}

function NodeSidebar({ node: n }: { node: MapNode }) {
  const ins = edges.filter((e) => e.to === n.id);
  const outs = edges.filter((e) => e.from === n.id);
  const fixes = FIXES[n.id];
  const bugs = KNOWN_BUGS[n.id];
  return (
    <>
      <h2>{n.label}</h2>
      {n.sub ? (
        <div style={{ color: 'var(--muted)', fontSize: '11px', marginBottom: '6px' }}>{n.sub}</div>
      ) : null}
      {n.role ? <div className="role">{n.role}</div> : null}
      {n.plain ? <div className="plain">{n.plain}</div> : null}
      {n.path ? <div className="path">{n.path}</div> : null}
      {bugs ? (
        <Field heading="Known bugs">
          {bugs.map((b, i) => (
            <div className="bug" key={i}>
              <span className="sev">{b.sev}</span>
              {b.t}
              <div style={{ opacity: 0.7, fontSize: '10px', marginTop: '3px' }}>{b.ref}</div>
            </div>
          ))}
        </Field>
      ) : null}
      {fixes ? (
        <Field heading="Roadmap / fixes">
          {fixes.map((f, i) => (
            <div className="fix" key={i}>
              #{f.n} · {f.t}
            </div>
          ))}
        </Field>
      ) : null}
      {n.notes.length > 0 ? (
        <Field heading="Notes">
          <ul>
            {n.notes.map((x, i) => (
              <li key={i}>{x}</li>
            ))}
          </ul>
        </Field>
      ) : null}
      <Field heading={`Wires in (${ins.length})`}>
        {ins.map((e, i) => (
          <div className="edgepair" key={i}>
            ← <b>{byId.get(e.from)?.label ?? e.from}</b> · {e.label ?? e.kind}
          </div>
        ))}
      </Field>
      <Field heading={`Wires out (${outs.length})`}>
        {outs.map((e, i) => (
          <div className="edgepair" key={i}>
            → <b>{byId.get(e.to)?.label ?? e.to}</b> · {e.label ?? e.kind}
          </div>
        ))}
      </Field>
      <Field heading="Tags">
        <div>
          {n.tag.map((t, i) => (
            <span className="tag" key={i}>
              {t}
            </span>
          ))}
        </div>
      </Field>
    </>
  );
}

export function Sidebar({ panel }: { panel: Panel }) {
  if (panel.kind === 'node') {
    const n = byId.get(panel.id);
    if (n) return <NodeSidebar node={n} />;
  }
  return <DefaultSidebar />;
}
