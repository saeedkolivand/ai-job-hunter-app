"use client";

// DOM chrome skeleton (M1, no final art): letterbox bars, the timecode
// (00:00 -> 02:40) and the depth gauge, driven from the store/playhead. The
// timecode + depth update every frame via refs (imperative textContent, NEVER
// per-frame React state); the act title is a discrete store field that changes a
// handful of times per session. Chrome is aria-hidden -- the accessible captions
// live in the a11y overlay's aria-live region.

import { useEffect, useRef } from "react";

import { letterboxFlex } from "@/engine/letterbox";
import { SCENES } from "@/engine/scene-resolver";
import { playhead, useRig } from "@/engine/store";
import {
  formatGauge,
  formatTimecode,
  gaugeMeters,
  isAltitudePhase,
  timecodeSeconds,
} from "@/engine/timecode";

// Peak extra letterbox bar height (px) at the splash impact. The flex is pure
// f(t) so it scrubs both directions with the playhead; the accompanying silence
// beat is an audio hook deferred to M5.
const LETTERBOX_FLEX_PX = 42;

// SSR-rendered initial text: computed from t=0 (never hand-duplicated) so the
// server-rendered markup always matches what the first client tick would write,
// and hydration never has to reconcile a stale hardcoded string here.
const INITIAL_TIMECODE = formatTimecode(0);
const INITIAL_GAUGE = formatGauge(0);

export function Chrome() {
  const chromeRef = useRef<HTMLDivElement>(null);
  const timecodeRef = useRef<HTMLSpanElement>(null);
  const gaugeRef = useRef<HTMLSpanElement>(null);
  const scene = useRig((s) => s.scene);
  const act = SCENES[scene]?.act ?? "";

  useEffect(() => {
    let raf = 0;
    // Cache the last NUMBER (+ phase), not the last formatted string: compare
    // on the cheap arithmetic value every rAF, and only allocate + write the
    // formatted string on the (much rarer) frame where it actually changed.
    let lastSeconds = -1;
    let lastGaugeM = Number.NaN;
    let lastPhase: boolean | null = null;
    let lastFlexPx = -1;
    const tick = (): void => {
      const t = playhead.t;
      const seconds = timecodeSeconds(t);
      if (seconds !== lastSeconds) {
        lastSeconds = seconds;
        if (timecodeRef.current) timecodeRef.current.textContent = formatTimecode(t);
      }
      const gaugeM = Math.round(gaugeMeters(t));
      const phase = isAltitudePhase(t);
      if (gaugeM !== lastGaugeM || phase !== lastPhase) {
        lastGaugeM = gaugeM;
        lastPhase = phase;
        if (gaugeRef.current) gaugeRef.current.textContent = formatGauge(t);
      }
      // Letterbox flex at the splash impact -- pushed to a CSS var (only on real
      // change) so the bars widen briefly then relax; the CSS composes it onto the
      // responsive base height, so the clamp() sizing is preserved.
      const flexPx = Math.round(letterboxFlex(t) * LETTERBOX_FLEX_PX);
      if (flexPx !== lastFlexPx) {
        lastFlexPx = flexPx;
        chromeRef.current?.style.setProperty("--lb-extra", `${flexPx}px`);
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);

  return (
    <div className="chrome" aria-hidden ref={chromeRef}>
      <div className="letterbox letterbox-top">
        <span className="act-title">{act}</span>
      </div>
      <div className="letterbox letterbox-bottom" />
      <div className="timecode">
        <span ref={timecodeRef}>{INITIAL_TIMECODE}</span>
      </div>
      <div className="depth-gauge">
        <span ref={gaugeRef}>{INITIAL_GAUGE}</span>
      </div>
    </div>
  );
}
