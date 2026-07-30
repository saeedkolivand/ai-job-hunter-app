'use client';

import { useState } from 'react';

import {
  AGENT_COUNT,
  type AgentRole,
  BY_NAME,
  CROSS_NODES,
  DIVIDER_FLEET,
  FLEET_LEGEND,
  PAIRS,
} from '@/data/agent-fleet';
import { copyChip, copyChipOnKeyDown } from '@/lib/agent-system/clipboard';

import { Divider } from '../Divider';
import { useMapLinks } from '../hooks';
import { NodeButton } from '../NodeButton';

const ROLE_LABEL: Record<AgentRole, string> = {
  author: 'WRITES',
  critic: 'AUDITS',
  cross: 'CROSS-CUTTING',
};

function roleClassOf(name: string): AgentRole {
  return BY_NAME.get(name)?.[1] ?? 'cross';
}

export function FleetMapSection() {
  const [selected, setSelected] = useState<string | null>(null);
  const [litName, setLitName] = useState<string | null>(null);
  const { gridRef, links, linksViewBox } = useMapLinks();

  const selectedTuple = selected ? BY_NAME.get(selected) : undefined;

  return (
    <>
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
                <NodeButton
                  name={author}
                  roleClass="author"
                  selected={selected === author}
                  onSelect={setSelected}
                  onHover={setLitName}
                />
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
                {critics.map((critic) => (
                  <NodeButton
                    key={critic}
                    name={critic}
                    roleClass="critic"
                    selected={selected === critic}
                    onSelect={setSelected}
                    onHover={setLitName}
                  />
                ))}
              </div>
            </div>
          ))}
          <div className="cross-band">
            cross-cutting &amp; risk — they ride along, no author pair
          </div>
          <div className="cross-row">
            {CROSS_NODES.map((name) => (
              <NodeButton
                key={name}
                name={name}
                roleClass={roleClassOf(name)}
                selected={selected === name}
                onSelect={setSelected}
                onHover={setLitName}
              />
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
                  {ROLE_LABEL[selectedTuple[1]]}
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
                onKeyDown={copyChipOnKeyDown}
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
        {FLEET_LEGEND.map((row) => (
          <span key={row.label}>
            <i className={row.swatchClass} />
            {row.label}
          </span>
        ))}
      </div>

      <Divider d={DIVIDER_FLEET} />
    </>
  );
}
