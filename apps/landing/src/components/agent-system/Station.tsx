import type { Station } from '@/data/agent-fleet';
import { STATION_COUNT } from '@/lib/agent-system/belt';

import { Machine } from './Machine';

// Renders one assembly-line station. `.belt-track` (horizontal, desktop) and
// `.belt-vert` (stacked, mobile — CSS media query driven) render the SAME
// station data in two different DOM shapes; both paths must exist since the
// media query switches between them, not React.
export interface StationViewProps {
  station: Station;
  index: number;
  layout: 'horizontal' | 'vertical';
}

export function StationView({ station, index, layout }: StationViewProps) {
  if (layout === 'vertical') {
    return (
      <li>
        <div className="vmachine">
          <Machine kind={station.machine} />
        </div>
        <div className="vbody">
          <div className="vn">
            station {index + 1} / {STATION_COUNT} · {station.access}
          </div>
          <div className="vt">{station.title}</div>
          <div className="vd">{station.desc}</div>
          {station.agentTag ? <span className="agent-tag">{station.agentTag}</span> : null}
        </div>
      </li>
    );
  }
  return (
    <div className={`station${index === 0 ? ' lit' : ''}`} data-i={index}>
      <div className="machine">
        <Machine kind={station.machine} />
      </div>
      <div className="post" />
      <div className="sn">
        {index + 1} / {STATION_COUNT}
      </div>
      <div className="st">{station.title}</div>
      <div className="sd">{station.desc}</div>
      {station.agentTag ? <span className="agent-tag">{station.agentTag}</span> : null}
    </div>
  );
}
