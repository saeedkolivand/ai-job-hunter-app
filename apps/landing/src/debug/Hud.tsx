"use client";

// Dev-only telemetry overlay for the gate-auditor: global t, active page, that
// page's p / exitP, fps, and the last frame's renderer.info.render.calls. A DOM
// element (never over canvas hit targets, pointer-events:none), driven by its
// own rAF poll reading the store + channels + hudStats -- it never re-renders
// React and never touches the GL loop. Gated to NODE_ENV development OR a ?hud=1
// param; renders nothing otherwise. window is only read inside an effect.

import { useEffect, useRef, useState } from "react";

import { channels } from "@/engine/channels";
import { hudStats } from "@/engine/stats";
import { ripbookStore } from "@/engine/store";

export default function Hud() {
  const [on, setOn] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const enabled =
      process.env.NODE_ENV === "development" ||
      new URLSearchParams(window.location.search).has("hud");
    if (enabled) setOn(true);
  }, []);

  useEffect(() => {
    if (!on) return;
    let raf = 0;
    let last = performance.now();
    let frames = 0;
    let acc = 0;
    let fps = 0;
    const tick = (now: number) => {
      const dt = now - last;
      last = now;
      frames += 1;
      acc += dt;
      if (acc >= 500) {
        fps = Math.round((frames * 1000) / acc);
        frames = 0;
        acc = 0;
      }
      const s = ripbookStore.getState();
      const ch = channels[s.activePage];
      const el = ref.current;
      if (el && ch) {
        el.textContent =
          `t ${s.t.toFixed(3)}  page ${s.activePage}  ` +
          `p ${ch.p.toFixed(2)}  exitP ${ch.exitP.toFixed(2)}  ` +
          `fps ${fps}  calls ${hudStats.calls}`;
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [on]);

  if (!on) return null;

  return (
    <div
      ref={ref}
      style={{
        position: "fixed",
        left: "8px",
        bottom: "8px",
        zIndex: 200,
        padding: "4px 8px",
        borderRadius: "6px",
        background: "rgba(8,10,16,0.72)",
        color: "#9fe8c2",
        font: "11px/1.4 monospace",
        letterSpacing: "0.02em",
        pointerEvents: "none",
        whiteSpace: "nowrap",
      }}
    />
  );
}
