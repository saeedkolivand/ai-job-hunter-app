'use client';

import {
  type MouseEvent as ReactMouseEvent,
  type ReactElement,
  useEffect,
  useRef,
  useState,
} from 'react';

import {
  AGENT_COUNT,
  type AgentRole,
  BY_NAME,
  CROSS_NODES,
  type MachineKind,
  PAIRS,
  type RouteCase,
  ROUTES,
  STATIONS,
} from '@/data/agent-fleet';

// ── inline SVG (ported verbatim from the hand-authored page) ─────────────────
function Glyph({ role }: { role: AgentRole }) {
  if (role === 'author') {
    return (
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path
          className="ink"
          style={{ stroke: 'currentColor' }}
          d="M5 19l2-6L17 3l3 3L10 16l-6 2z"
        />
        <path className="ink" style={{ stroke: 'currentColor' }} d="M14 6l3 3" />
      </svg>
    );
  }
  if (role === 'critic') {
    return (
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <circle className="ink" style={{ stroke: 'currentColor' }} cx="10" cy="10" r="6" />
        <path className="ink" style={{ stroke: 'currentColor' }} d="M14.5 14.5L20 20" />
      </svg>
    );
  }
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path
        className="ink"
        style={{ stroke: 'currentColor' }}
        d="M12 3l8 4v5c0 5-4 8-8 9-4-1-8-4-8-9V7z"
      />
    </svg>
  );
}

const MACHINE_PATHS: Record<MachineKind, ReactElement> = {
  router: <path className="ink ink-a" d="M12 30h28M26 16v28M14 22l-2-6 6 2M38 22l2-6-6 2" />,
  pen: (
    <>
      <path className="ink ink-g" d="M14 44l4-10L40 12l6 6L24 40z" />
      <path className="ink ink-g" d="M36 16l6 6" />
    </>
  ),
  mag: (
    <>
      <circle className="ink ink-p" cx="24" cy="24" r="13" />
      <path className="ink ink-p" d="M34 34l10 10" />
    </>
  ),
  tube: <path className="ink ink-a" d="M22 8h12M25 8v22a7 7 0 0014 0V8M26 26h12" />,
  broom: (
    <path
      className="ink ink-a"
      d="M30 8L18 30M14 42l8-14 12 6-6 14zM12 44l4-6M18 46l4-6M24 48l4-6"
    />
  ),
  quill: (
    <path className="ink ink-a" d="M40 10C24 14 14 28 14 42M14 42c10-2 22-10 26-22M20 36h10" />
  ),
  gate: <path className="ink ink-r" d="M12 40V14h24v26M12 22h24M20 14v26M28 14v26" />,
  rocket: (
    <path
      className="ink ink-a"
      d="M24 8c8 6 10 16 8 26l-8 6-8-6c-2-10 0-20 8-26zM24 24a3 3 0 100-6 3 3 0 000 6M18 34l-4 8 8-3M30 34l4 8-8-3"
    />
  ),
};

function Machine({ kind }: { kind: MachineKind }) {
  return (
    <svg viewBox="0 0 56 56" aria-hidden="true">
      {MACHINE_PATHS[kind]}
    </svg>
  );
}

function Divider({ d }: { d: string }) {
  return (
    <div className="divider draw" aria-hidden="true">
      <svg viewBox="0 0 620 16" preserveAspectRatio="none">
        <path className="ink" d={d} />
      </svg>
    </div>
  );
}

// click-to-copy: copy the chip's data-copy, briefly flag it "copied"
function copyChip(event: ReactMouseEvent<HTMLElement>): void {
  const el = event.currentTarget;
  const text = el.getAttribute('data-copy') ?? el.textContent ?? '';
  if (!navigator.clipboard) return;
  void navigator.clipboard.writeText(text).then(() => {
    el.classList.add('copied');
    window.setTimeout(() => el.classList.remove('copied'), 1300);
  });
}

// ── role → detail-panel copy ─────────────────────────────────────────────────
function roleClassOf(name: string): AgentRole {
  return BY_NAME.get(name)?.[1] ?? 'cross';
}

export function AgentFleet() {
  const [selected, setSelected] = useState<string | null>(null);
  const [route, setRoute] = useState<RouteCase | null>(null);
  const [links, setLinks] = useState<{ pair: string; d: string }[]>([]);
  const [litName, setLitName] = useState<string | null>(null);
  const [linksViewBox, setLinksViewBox] = useState('0 0 0 0');

  const rootRef = useRef<HTMLDivElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const beltSectionRef = useRef<HTMLDivElement>(null);
  const trackRef = useRef<HTMLDivElement>(null);
  const stepRef = useRef<HTMLElement>(null);
  const nameRef = useRef<HTMLElement>(null);
  const labelRef = useRef<HTMLSpanElement>(null);
  const stampRef = useRef<HTMLSpanElement>(null);

  // ── fleet-map link geometry (layout-dependent → measured in an effect, then
  //    rendered declaratively as <path> state; hover lighting is React state) ──
  useEffect(() => {
    function buildLinks() {
      const grid = gridRef.current;
      if (!grid) return;
      if (window.matchMedia('(max-width:780px)').matches) {
        setLinks([]);
        return;
      }
      const gr = grid.getBoundingClientRect();
      setLinksViewBox(`0 0 ${gr.width} ${gr.height}`);
      const out: { pair: string; d: string }[] = [];
      for (const [author, critics] of PAIRS) {
        const aEl = grid.querySelector<HTMLElement>(`.node[data-name="${author}"]`);
        if (!aEl) continue;
        const ar = aEl.getBoundingClientRect();
        const ax = ar.right - gr.left;
        const ay = ar.top + ar.height / 2 - gr.top;
        for (const critic of critics) {
          const cEl = grid.querySelector<HTMLElement>(`.node[data-name="${critic}"]`);
          if (!cEl) continue;
          const cr = cEl.getBoundingClientRect();
          const cx = cr.left - gr.left;
          const cy = cr.top + cr.height / 2 - gr.top;
          const mx = (ax + cx) / 2;
          out.push({
            pair: `${author}|${critic}`,
            d: `M${ax} ${ay} C${mx} ${ay} ${mx} ${cy} ${cx} ${cy}`,
          });
        }
      }
      setLinks(out);
    }

    buildLinks();
    let resizeTimer = 0;
    const onResize = () => {
      window.clearTimeout(resizeTimer);
      resizeTimer = window.setTimeout(buildLinks, 160);
    };
    window.addEventListener('resize', onResize);
    if (document.fonts?.ready) void document.fonts.ready.then(buildLinks);

    return () => {
      window.removeEventListener('resize', onResize);
      window.clearTimeout(resizeTimer);
    };
  }, []);

  // ── belt scroll scrub (layout-dependent → imperative) ────────────────────────
  useEffect(() => {
    const section = beltSectionRef.current;
    const track = trackRef.current;
    if (!section || !track) return;
    const viewport = track.parentElement;
    let ticking = false;
    let lastStep = -1;

    const horizontal = () => !window.matchMedia('(max-width:780px)').matches;

    function paintBelt() {
      ticking = false;
      if (!section || !track || !horizontal()) return;
      const rect = section.getBoundingClientRect();
      const runway = section.offsetHeight - window.innerHeight;
      const scrolled = Math.min(runway, Math.max(0, -rect.top));
      const progress = runway > 0 ? scrolled / runway : 0;
      const stations = Array.from(track.querySelectorAll<HTMLElement>('.station'));
      const first = stations[0];
      const last = stations[stations.length - 1];
      if (!first || !last) return;

      const firstC = first.offsetLeft + first.offsetWidth / 2;
      const lastC = last.offsetLeft + last.offsetWidth / 2;
      const activeX = firstC + (lastC - firstC) * progress;
      const center = (viewport?.clientWidth ?? window.innerWidth) / 2;
      track.style.transform = `translateX(${(center - activeX).toFixed(1)}px)`;

      let step = 0;
      let minDiff = Infinity;
      stations.forEach((s, i) => {
        const c = s.offsetLeft + s.offsetWidth / 2;
        const diff = Math.abs(c - activeX);
        if (diff < minDiff) {
          minDiff = diff;
          step = i;
        }
      });
      stations.forEach((s, i) => s.classList.toggle('lit', i === step));

      if (step !== lastStep) {
        lastStep = step;
        const st = STATIONS[step];
        if (st) {
          if (stepRef.current) stepRef.current.textContent = String(step + 1);
          if (nameRef.current) nameRef.current.textContent = st.title;
          if (labelRef.current) labelRef.current.textContent = st.title;
          if (stampRef.current) stampRef.current.textContent = st.stamp;
        }
      }
    }

    const schedule = () => {
      if (!ticking) {
        ticking = true;
        window.requestAnimationFrame(paintBelt);
      }
    };
    window.addEventListener('scroll', schedule, { passive: true });
    window.addEventListener('resize', schedule);
    if (document.fonts?.ready) void document.fonts.ready.then(paintBelt);
    paintBelt();

    return () => {
      window.removeEventListener('scroll', schedule);
      window.removeEventListener('resize', schedule);
    };
  }, []);

  // ── reveal-on-scroll, draw-on-scroll, count-up ───────────────────────────────
  useEffect(() => {
    const root = rootRef.current;
    if (!root || !('IntersectionObserver' in window)) return;
    const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

    const countEl = root.querySelector<HTMLElement>('[data-count]');
    function countUp(el: HTMLElement) {
      const to = Number(el.getAttribute('data-count')) || 0;
      if (reduce) {
        el.textContent = String(to);
        return;
      }
      const duration = 900;
      let start: number | null = null;
      function frame(t: number) {
        if (start === null) start = t;
        const p = Math.min(1, (t - start) / duration);
        const eased = 1 - Math.pow(1 - p, 3);
        el.textContent = String(Math.round(eased * to));
        if (p < 1) window.requestAnimationFrame(frame);
        else el.textContent = String(to);
      }
      el.textContent = '0';
      window.requestAnimationFrame(frame);
    }

    const io = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          entry.target.classList.add('in');
          if (entry.target === countEl) countUp(countEl);
          io.unobserve(entry.target);
        }
      },
      { threshold: 0.16, rootMargin: '0px 0px -8% 0px' }
    );
    root.querySelectorAll('.reveal').forEach((el) => io.observe(el));
    if (countEl) io.observe(countEl);

    const scenes = Array.from(root.querySelectorAll<HTMLElement>('.draw'));
    let ticking = false;
    function paintScenes() {
      const vh = window.innerHeight;
      for (const scene of scenes) {
        const r = scene.getBoundingClientRect();
        let p = (vh - r.top) / (vh + r.height);
        p = p < 0 ? 0 : p > 1 ? 1 : p;
        scene.style.setProperty('--p', p.toFixed(4));
      }
      ticking = false;
    }
    const schedule = () => {
      if (!ticking) {
        ticking = true;
        window.requestAnimationFrame(paintScenes);
      }
    };
    window.addEventListener('scroll', schedule, { passive: true });
    window.addEventListener('resize', schedule);
    paintScenes();

    return () => {
      io.disconnect();
      window.removeEventListener('scroll', schedule);
      window.removeEventListener('resize', schedule);
    };
  }, []);

  const selectedTuple = selected ? BY_NAME.get(selected) : undefined;

  function NodeButton({ name, roleClass }: { name: string; roleClass: AgentRole }) {
    return (
      <button
        type="button"
        className={`node ${roleClass}${roleClass === 'author' ? ' node-author' : ''}`}
        data-name={name}
        aria-expanded={selected === name}
        onClick={() => setSelected(name)}
        onMouseEnter={() => setLitName(name)}
        onMouseLeave={() => setLitName(null)}
        onFocus={() => setLitName(name)}
        onBlur={() => setLitName(null)}
      >
        <span className="ng">
          <Glyph role={roleClass} />
        </span>
        <span>{name}</span>
      </button>
    );
  }

  return (
    <div className="agent-fleet" ref={rootRef}>
      {/* 1 · HERO */}
      <p className="scrawl reveal">the .claude/ agent system</p>
      <h1 className="reveal">The Agent Fleet</h1>
      <p className="tag reveal">
        a swarm of tiny specialists builds this repo, and a second swarm{' '}
        <em>tears their work apart.</em>
      </p>
      <p className="count reveal">
        <span className="num" data-count={AGENT_COUNT}>
          {AGENT_COUNT}
        </span>{' '}
        <b>agents</b> · paired author + critic per domain · nobody approves their own work.
      </p>

      <div className="hero-scene reveal draw" aria-hidden="true">
        <svg viewBox="0 0 560 220">
          <g className="glow">
            <rect className="ink ink-g" x="40" y="70" width="120" height="92" rx="14" />
            <circle className="ink ink-g" cx="74" cy="104" r="8" />
            <circle className="ink ink-g" cx="126" cy="104" r="8" />
            <path className="ink ink-g" d="M78 134 q22 12 44 0" />
            <path className="ink ink-g" d="M100 70 V52" />
            <circle className="ink-fill" cx="100" cy="48" r="5" style={{ fill: 'var(--author)' }} />
            <path className="ink ink-g" d="M150 120 l34 14" />
            <path className="ink ink-g" d="M182 128 l14 10 -10 6 z" />
          </g>
          <text className="scene-cap a" x="100" y="190" textAnchor="middle">
            author · writes
          </text>
          <g className="glow">
            <rect className="ink ink-a" x="250" y="92" width="60" height="74" rx="7" />
            <path className="ink ink-g no-draw" d="M262 112 h36" style={{ opacity: 0.8 }} />
            <path className="ink ink-r no-draw" d="M262 126 h26" style={{ opacity: 0.8 }} />
            <path className="ink ink-g no-draw" d="M262 140 h32" style={{ opacity: 0.8 }} />
          </g>
          <path
            className="ink no-draw"
            strokeDasharray="4 7"
            d="M196 150 q90 30 150 -6"
            style={{ opacity: 0.5 }}
          />
          <g className="glow">
            <rect className="ink ink-p" x="400" y="70" width="120" height="92" rx="14" />
            <path className="ink ink-p" d="M424 100 q12 -8 24 0" />
            <circle className="ink ink-p" cx="436" cy="108" r="7" />
            <circle className="ink ink-p" cx="488" cy="106" r="11" />
            <path className="ink ink-p" d="M496 114 l10 10" />
            <path className="ink ink-p" d="M428 140 q24 -8 48 0" />
          </g>
          <text className="scene-cap c" x="460" y="190" textAnchor="middle">
            critic · audits
          </text>
        </svg>
      </div>

      <p className="lede reveal">
        Every task and code change routes through the <b>.claude/</b> system. You name no agent. It
        reads the files you touched, picks the <b>author</b> that owns that area, runs an{' '}
        <b>independent critic</b> over the diff, pulls in a security or performance <b>secondary</b>{' '}
        when there&rsquo;s risk, writes tests, sweeps dead code, and lets a <b>steward</b> close the
        docs. Poke around below to watch it move.
      </p>

      <Divider d="M2 9 Q160 2 320 8 T618 7" />

      {/* 2 · INTAKE → DELEGATION */}
      <p className="scrawl reveal">one issue in →</p>
      <h2 className="section reveal">Intake → Delegation</h2>
      <p className="section-sub reveal">the fleet decides who touches it, and why.</p>

      <div className="intake">
        <div className="issue-list reveal" role="group" aria-label="sample issues to route">
          {ROUTES.map((r) => (
            <button
              key={r.id}
              className="issue"
              type="button"
              aria-pressed={route?.id === r.id}
              onClick={() => setRoute(r)}
            >
              <svg className="gi" viewBox="0 0 24 24" aria-hidden="true">
                <rect className="ink" x="4" y="7" width="16" height="10" rx="2" />
                <path className="ink" d="M9 12h6" />
              </svg>
              {r.issue}
            </button>
          ))}
        </div>
        <div className="route reveal" aria-live="polite" aria-atomic="true">
          {route ? (
            <>
              <h3>{route.title}</h3>
              <p className="area">{route.area}</p>
              <ul className="flow">
                {route.rows.map((row, i) =>
                  row.kind === 'area' ? (
                    <li className="area" key={i}>
                      <span className="lbl">area</span>
                      <span className="why">{row.detail}</span>
                    </li>
                  ) : (
                    <li key={i}>
                      <span className="lbl">
                        {row.kind === 'author'
                          ? 'author'
                          : row.kind === 'secondary'
                            ? 'secondary'
                            : row.kind === 'gate'
                              ? 'gate'
                              : 'critic'}
                      </span>
                      <span className="who">
                        <b
                          className={
                            row.kind === 'author'
                              ? ''
                              : row.kind === 'secondary'
                                ? 'sec'
                                : row.kind === 'gate'
                                  ? 'gate'
                                  : 'crit'
                          }
                        >
                          {row.name}
                        </b>
                      </span>
                      <span className="why">— {row.why}</span>
                    </li>
                  )
                )}
              </ul>
            </>
          ) : (
            <p className="placeholder">pick an issue to see who handles it.</p>
          )}
        </div>
      </div>

      <Divider d="M2 8 Q160 14 320 7 T618 9" />

      {/* 3 · THE ASSEMBLY LINE */}
      <div className="belt-section" ref={beltSectionRef} aria-labelledby="belt-title">
        <div className="belt-sticky">
          <div className="belt-head">
            <p className="scrawl">one change, down the line</p>
            <h2 className="section" id="belt-title">
              The Assembly Line
            </h2>
            <p className="section-sub">
              a single diff rides through nine stations, each a different specialist. scroll to run
              it.
            </p>
          </div>
          <div className="belt-viewport" aria-hidden="true">
            <div className="belt-rail">
              <svg preserveAspectRatio="none" viewBox="0 0 1000 26">
                <path className="ink ink-a" style={{ opacity: 0.7 }} d="M0 6 H1000" />
                <path className="ink ink-a" style={{ opacity: 0.4 }} d="M0 20 H1000" />
              </svg>
            </div>
            <div className="belt-track" ref={trackRef}>
              {STATIONS.map((st, i) => (
                <div className={`station${i === 0 ? ' lit' : ''}`} data-i={i} key={st.title + i}>
                  <div className="machine">
                    <Machine kind={st.machine} />
                  </div>
                  <div className="post" />
                  <div className="sn">{i + 1} / 9</div>
                  <div className="st">{st.title}</div>
                  <div className="sd">{st.desc}</div>
                  {st.agentTag ? <span className="agent-tag">{st.agentTag}</span> : null}
                </div>
              ))}
            </div>
            <div className="diff-token">
              <div className="diff-card">
                <span className="fn">match_resume.rs</span>
                <span className="ln add" />
                <span className="ln" />
                <span className="ln del" />
                <span className="ln add" />
                <span className="diff-stamp" ref={stampRef}>
                  ·
                </span>
              </div>
              <span className="label" ref={labelRef}>
                intake
              </span>
            </div>
          </div>
          <p className="belt-progress">
            station <b ref={stepRef}>1</b> / 9 · <span ref={nameRef}>intake &amp; triage</span>
          </p>
          <p className="belt-hint">↓ keep scrolling — the diff moves with you</p>
        </div>
        <div className="belt-vert">
          <div className="belt-head" style={{ textAlign: 'left', paddingLeft: 0 }}>
            <p className="scrawl">one change, down the line</p>
            <h2 className="section">The Assembly Line</h2>
            <p className="section-sub">
              a single diff passes through nine stations, each a different specialist.
            </p>
          </div>
          <ol>
            {STATIONS.map((st, i) => (
              <li key={st.title + i}>
                <div className="vmachine">
                  <Machine kind={st.machine} />
                </div>
                <div className="vbody">
                  <div className="vn">
                    station {i + 1} / 9 · {st.access}
                  </div>
                  <div className="vt">{st.title}</div>
                  <div className="vd">{st.desc}</div>
                  {st.agentTag ? <span className="agent-tag">{st.agentTag}</span> : null}
                </div>
              </li>
            ))}
          </ol>
        </div>
      </div>

      <Divider d="M2 9 Q160 3 320 9 T618 7" />

      {/* 4 · FLEET MAP = ROSTER */}
      <p className="scrawl reveal">every specialist, one tap away</p>
      <h2 className="section reveal">The Fleet</h2>
      <p className="section-sub reveal">
        authors write (left, green). critics audit (right, pink). tap any node for its brief: pair,
        paths, and how you&rsquo;d delegate to it.
      </p>

      <div className="map-wrap reveal">
        <div className="constellation" ref={gridRef}>
          <div className="col-head a">authors ✎</div>
          <div className="col-head c">⌕ critics</div>
          {PAIRS.map(([author, critics]) => (
            <div className="pair-row" key={author}>
              <div>
                <NodeButton name={author} roleClass="author" />
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
                {critics.map((critic) => (
                  <NodeButton key={critic} name={critic} roleClass="critic" />
                ))}
              </div>
            </div>
          ))}
          <div className="cross-band">
            cross-cutting &amp; risk — they ride along, no author pair
          </div>
          <div className="cross-row">
            {CROSS_NODES.map((name) => (
              <NodeButton key={name} name={name} roleClass={roleClassOf(name)} />
            ))}
          </div>
          <svg
            className="map-links"
            viewBox={linksViewBox}
            aria-hidden="true"
            preserveAspectRatio="none"
          >
            {links.map((link) => (
              <path
                key={link.pair}
                data-pair={link.pair}
                className={litName !== null && link.pair.split('|').includes(litName) ? 'lit' : ''}
                d={link.d}
              />
            ))}
          </svg>
        </div>
        <div className="detail" aria-live="polite" aria-atomic="true">
          {selectedTuple ? (
            <>
              <div className="d-top">
                <span className={`role-pill role-${selectedTuple[1]}`}>
                  {selectedTuple[1] === 'author'
                    ? 'WRITES'
                    : selectedTuple[1] === 'critic'
                      ? 'AUDITS'
                      : 'CROSS-CUTTING'}
                </span>
                <span className="d-name">{selectedTuple[0]}</span>
              </div>
              <p className="d-role">{selectedTuple[2]}</p>
              <p className="d-meta">{selectedTuple[3]}</p>
              <p className="d-meta">📁 {selectedTuple[4]}</p>
              <span
                className="copy-cmd"
                role="button"
                tabIndex={0}
                title="click to copy"
                data-copy={selectedTuple[5]}
                onClick={copyChip}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    copyChip(e as unknown as ReactMouseEvent<HTMLElement>);
                  }
                }}
              >
                {selectedTuple[5]}
              </span>
            </>
          ) : (
            <p className="placeholder">
              tap a node above — every one of the {AGENT_COUNT} is here.
            </p>
          )}
        </div>
      </div>
      <div className="legend">
        <span>
          <i className="sw author" /> author (writes)
        </span>
        <span>
          <i className="sw critic" /> critic (audits)
        </span>
        <span>
          <i className="sw cross" /> cross-cutting / risk
        </span>
        <span>
          <i className="sw line" /> author ↔ critic pairing
        </span>
        <span>
          <i className="sw line risk" /> risk secondary
        </span>
      </div>

      <Divider d="M2 8 Q160 13 320 7 T618 9" />

      {/* 5 · WORK TOGETHER */}
      <p className="scrawl reveal">all at once →</p>
      <h2 className="section reveal">Make Them Work Together</h2>
      <p className="section-sub reveal">one cross-layer ask, the whole fleet in parallel.</p>

      <div className="big-prompt reveal">
        <span
          className="copy-cmd"
          role="button"
          tabIndex={0}
          title="click to copy"
          data-copy="Implement a 'save job to a folder' feature end-to-end: new IPC command + Rust store, a renderer panel to pick the folder, and ATS-aware tagging. Route it through the agent fleet."
          onClick={copyChip}
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              copyChip(e as unknown as ReactMouseEvent<HTMLElement>);
            }
          }}
        >
          Implement a &quot;save job to a folder&quot; feature end-to-end: new IPC command + Rust
          store, a renderer panel to pick the folder, and ATS-aware tagging. Route it through the
          agent fleet.
        </span>
        <div className="teams">
          <div className="team">
            <h4>Disjoint files, in parallel</h4>
            <ul>
              <li>
                <code>rust-backend-author</code> — IPC command + SQLite store (
                <code>commands/</code>, <code>data_store.rs</code>)
              </li>
              <li>
                <code>frontend-author</code> — the folder-picker panel (<code>renderer/**</code>)
              </li>
              <li>
                <code>job-match-author</code> — the ATS tag logic
              </li>
            </ul>
          </div>
          <div className="team">
            <h4>Critics challenge each other</h4>
            <ul>
              <li>
                <code>rust-backend-architect</code> + <code>tauri-security-reviewer</code> on the
                new IPC surface
              </li>
              <li>
                <code>frontend-reviewer</code> + <code>ui-ux-expert</code> on the panel
              </li>
              <li>
                <code>job-match-expert</code> on the scoring
              </li>
            </ul>
          </div>
          <div className="team">
            <h4>Then the close-out</h4>
            <ul>
              <li>
                <code>test-author</code> → <code>testing-reviewer</code> across all three
              </li>
              <li>
                <code>cleanup</code> sweep
              </li>
              <li>
                <code>project-steward</code> docs + lessons
              </li>
            </ul>
          </div>
        </div>
      </div>

      <Divider d="M2 9 Q160 2 320 8 T618 8" />

      {/* 6 · WITH vs WITHOUT */}
      <p className="scrawl reveal">is it worth it?</p>
      <h2 className="section reveal">With vs Without the Fleet</h2>
      <p className="section-sub reveal">
        same job. honestly: agents cost a token premium for higher reliability + parallel speed.
      </p>

      <div className="versus">
        <div className="vcol without reveal">
          <h3>One agent, no fleet</h3>
          <div className="dim">
            <span className="k">Wall-clock</span>
            <span className="v">
              Serial. One context does intake, code, review, tests, docs — back to back.
            </span>
          </div>
          <div className="dim">
            <span className="k">Tokens</span>
            <span className="v">
              Cold re-exploration every spawn: <b>≈70–122k tokens/spawn</b>, <b>≈120 tool-uses</b>{' '}
              to rebuild context it never kept.
            </span>
          </div>
          <div className="dim">
            <span className="k">Bugs caught before release</span>
            <span className="v">
              The writer reviews its own work — the missed error is the one it didn&rsquo;t see the
              first time. No release gate.
            </span>
          </div>
        </div>
        <div className="vcol with reveal">
          <h3>
            The agent fleet <span className="ink-badge">↑ this one</span>
          </h3>
          <div className="dim">
            <span className="k">Wall-clock</span>
            <span className="v">
              Authors run in parallel on disjoint files; critics audit alongside. Faster on
              cross-layer work.
            </span>
          </div>
          <div className="dim">
            <span className="k">Tokens</span>
            <span className="v">
              Pre-harvested handoff instead of cold start: <b>≈44 tool-uses</b> vs ≈120. Multi-agent
              adds <b>≈+58% token overhead</b> — kept small by the handoff.
            </span>
          </div>
          <div className="dim">
            <span className="k">Bugs caught before release</span>
            <span className="v">
              An independent writer/critic split catches <b>≈60–80%</b> of otherwise-missed errors,
              plus the Stop review-gate before merge.
            </span>
          </div>
        </div>
      </div>
      <p className="illustrative reveal">
        <b>ILLUSTRATIVE FIGURES.</b> The numbers above (cold-start ≈70–122k tokens/spawn; ≈44 vs
        ≈120 tool-uses pre-harvested vs cold; writer/critic catching ≈60–80% of missed errors;
        multi-agent ≈+58% token overhead) are illustrative, not measured benchmarks of this repo.
        The honest trade: agents cost a token premium for higher reliability and parallel speed; the
        pre-harvested handoff is what keeps that premium small.
      </p>

      <Divider d="M2 8 Q160 13 320 7 T618 9" />
    </div>
  );
}
