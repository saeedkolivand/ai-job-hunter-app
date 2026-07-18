"use client";

// DOM chrome skeleton (M1, no final art): letterbox bars, the timecode
// (00:00 -> 02:40) and the depth gauge, driven from the store/playhead. The
// timecode + depth update every frame via refs (imperative textContent, NEVER
// per-frame React state); the act title is a discrete store field that changes a
// handful of times per session. Chrome is aria-hidden -- the accessible captions
// live in the a11y overlay's aria-live region.

import { useEffect, useRef } from "react";

import { SCENES } from "@/engine/scene-resolver";
import { playhead, useRig } from "@/engine/store";
import { depthMeters, formatDepth, formatTimecode, timecodeSeconds } from "@/engine/timecode";

export function Chrome() {
  const timecodeRef = useRef<HTMLSpanElement>(null);
  const depthRef = useRef<HTMLSpanElement>(null);
  const scene = useRig((s) => s.scene);
  const act = SCENES[scene]?.act ?? "";

  useEffect(() => {
    let raf = 0;
    // Cache the last NUMBER, not the last formatted string: compare on the
    // cheap arithmetic value every rAF, and only allocate + write the
    // formatted string on the (much rarer) frame where it actually changed.
    let lastSeconds = -1;
    let lastDepthM = -1;
    const tick = (): void => {
      const t = playhead.t;
      const seconds = timecodeSeconds(t);
      if (seconds !== lastSeconds) {
        lastSeconds = seconds;
        if (timecodeRef.current) timecodeRef.current.textContent = formatTimecode(t);
      }
      const depthM = Math.round(depthMeters(t));
      if (depthM !== lastDepthM) {
        lastDepthM = depthM;
        if (depthRef.current) depthRef.current.textContent = formatDepth(t);
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);

  return (
    <div className="chrome" aria-hidden>
      <div className="letterbox letterbox-top">
        <span className="act-title">{act}</span>
      </div>
      <div className="letterbox letterbox-bottom" />
      <div className="timecode">
        <span ref={timecodeRef}>00:00</span>
      </div>
      <div className="depth-gauge">
        DEPTH <span ref={depthRef}>0 m</span>
      </div>
    </div>
  );
}
