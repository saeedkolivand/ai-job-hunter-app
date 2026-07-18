// The rig store. The playhead is a PREALLOCATED object mutated in place every
// frame -- GL and chrome read it via getState() (or the exported singleton),
// never through a hook selector (a selector would re-render the tree every
// frame). Only the discrete fields (mode / scene / tier / motionReduced) flow
// through set() and legitimately drive React re-renders; those change a handful
// of times per session, not per frame.

import { create } from "zustand";

export type RigMode = "pending" | "gl-live" | "slideshow" | "fallback";
export type QualityTier = "HIGH" | "MID" | "LOW";

// Mutated in place by the scroll rig each frame. Stable object identity, so
// writing .t never notifies subscribers.
export interface Playhead {
  t: number; // [0, 1]
  velocity: number; // playhead units per second (signed)
  scene: number; // 0..8
  sceneProgress: number; // sp within the active scene [0, 1]
}

export interface RigStore {
  readonly playhead: Playhead;
  mode: RigMode;
  scene: number; // discrete mirror of playhead.scene; set only on scene change
  tier: QualityTier;
  motionReduced: boolean;
  setMode: (mode: RigMode) => void;
  setScene: (scene: number) => void;
  setTier: (tier: QualityTier) => void;
  setMotionReduced: (motionReduced: boolean) => void;
}

export const useRig = create<RigStore>((set) => ({
  playhead: { t: 0, velocity: 0, scene: 0, sceneProgress: 0 },
  mode: "pending",
  scene: 0,
  tier: "HIGH",
  motionReduced: false,
  setMode: (mode) => set({ mode }),
  setScene: (scene) => set({ scene }),
  setTier: (tier) => set({ tier }),
  setMotionReduced: (motionReduced) => set({ motionReduced }),
}));

// Convenience singleton for per-frame reads: `playhead.t` inside a useFrame /
// rAF loop. Identity is fixed for the session.
export const playhead = useRig.getState().playhead;
