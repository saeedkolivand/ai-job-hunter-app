'use client';

import { useState } from 'react';

import { DIVIDER_INTAKE, type RouteCase, ROUTES } from '@/data/agent-fleet';

import { Divider } from '../Divider';

// Route-row `kind` → the `<b>` class it gets in the "who" column. The label
// text is `row.kind` itself (author/critic/secondary/gate all label
// themselves), so only the class needs a lookup.
const ROUTE_ROW_CLASS: Record<'author' | 'critic' | 'secondary' | 'gate', string> = {
  author: '',
  critic: 'crit',
  secondary: 'sec',
  gate: 'gate',
};

export function IntakeSection() {
  const [route, setRoute] = useState<RouteCase | null>(null);

  return (
    <>
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
                      <span className="lbl">{row.kind}</span>
                      <span className="who">
                        <b className={ROUTE_ROW_CLASS[row.kind]}>{row.name}</b>
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

      <Divider d={DIVIDER_INTAKE} />
    </>
  );
}
