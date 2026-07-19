"use client";

// The R3F canvas host. Fixed full-viewport, BEHIND the chrome + a11y overlays;
// the tall Semantic layer provides the scroll height so the page scrolls while
// this canvas stays put. The canvas is aria-hidden -- the a11y overlay is the
// accessible interface while GL runs.

import { Canvas } from "@react-three/fiber";

import { dprCap } from "@/engine/experience-gate";
import type { QualityTier } from "@/engine/store";

import { CanyonWorld } from "./world/CanyonWorld";

export function GlCanvas({ tier }: { tier: QualityTier }) {
  return (
    <div
      aria-hidden
      style={{ position: "fixed", inset: 0, zIndex: 1, pointerEvents: "none" }}
    >
      <Canvas
        dpr={[1, dprCap(tier)]}
        gl={{ antialias: true, powerPreference: "high-performance" }}
        camera={{ position: [0, 0, 6], fov: 55 }}
      >
        <CanyonWorld tier={tier} />
      </Canvas>
    </div>
  );
}
