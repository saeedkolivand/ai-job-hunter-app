import { DIVIDER_BELT, STATIONS } from '@/data/agent-fleet';
import { STATION_COUNT } from '@/lib/agent-system/belt';

import { Divider } from '../Divider';
import { useBeltScrub } from '../hooks';
import { StationView } from '../Station';

export function BeltSection() {
  const { beltSectionRef, trackRef, stepRef, nameRef, labelRef, stampRef } = useBeltScrub();

  return (
    <>
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
                <StationView key={st.title + i} station={st} index={i} layout="horizontal" />
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
            station <b ref={stepRef}>1</b> / {STATION_COUNT} ·{' '}
            <span ref={nameRef}>intake &amp; triage</span>
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
              <StationView key={st.title + i} station={st} index={i} layout="vertical" />
            ))}
          </ol>
        </div>
      </div>

      <Divider d={DIVIDER_BELT} />
    </>
  );
}
