// The single Experience gate (ADR-0014, still binding): decides whether GL
// mounts at all. Fail ANY of WebGL2 / fine pointer / CSS width > 900px / NOT
// reduced-motion / detect-gpu tier > 0 -> the prerendered Semantic layer runs
// and GL never mounts. All window/document access is inside functions (never at
// module scope) so this stays SSR-safe.

import { getGPUTier } from "detect-gpu";

import { NARROW_BREAKPOINT_PX } from "./constants";
import type { QualityTier } from "./store";

export interface Capabilities {
  webgl2: boolean;
  finePointer: boolean;
  wideViewport: boolean;
  reducedMotion: boolean;
  gpuTier: number; // 0..3 from detect-gpu
}

function hasWebGL2(): boolean {
  try {
    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("webgl2");
    return ctx != null;
  } catch {
    return false;
  }
}

// Read the current capabilities. detect-gpu is async (it may micro-benchmark),
// so this returns a promise; everything else is a synchronous media/feature
// query.
export async function probeCapabilities(): Promise<Capabilities> {
  const finePointer = window.matchMedia("(pointer: fine)").matches;
  const wideViewport = window.innerWidth > NARROW_BREAKPOINT_PX;
  const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const webgl2 = hasWebGL2();

  let gpuTier: number;
  try {
    const result = await getGPUTier();
    gpuTier = result.tier ?? 0;
  } catch {
    gpuTier = 0;
  }

  return { webgl2, finePointer, wideViewport, reducedMotion, gpuTier };
}

// The gate verdict. All conditions must hold for GL to mount.
export function gatePasses(c: Capabilities): boolean {
  return c.webgl2 && c.finePointer && c.wideViewport && !c.reducedMotion && c.gpuTier > 0;
}

// Startup quality tier from the detect-gpu tier: 3 -> HIGH, 2 -> MID, 1 -> LOW.
// (tier 0 fails the gate, so the caller never asks for a quality below LOW.)
export function tierToQuality(gpuTier: number): QualityTier {
  if (gpuTier >= 3) return "HIGH";
  if (gpuTier === 2) return "MID";
  return "LOW";
}

// The dpr cap per tier (skill's per-tier ladder, starting values).
export function dprCap(tier: QualityTier): number {
  switch (tier) {
    case "HIGH":
      return 2;
    case "MID":
      return 1.5;
    case "LOW":
      return 1;
  }
}
