// Journey state: the single vanilla zustand store the whole GL engine reads
// per-frame. scroll.ts is the ONLY writer (via ScrollTrigger.onUpdate); every
// consumer (camera, beats, effects) reads journeyStore.getState() inside its
// useFrame -- never a React selector in a hot path, which would re-render on
// every scroll tick. Two fields only: t (global progress in [0,1], the t-space
// authority) and vel (Lenis scroll velocity, for motion-reactive effects).

import { createStore } from "zustand/vanilla";

export interface JourneyState {
  t: number;
  vel: number;
}

export const journeyStore = createStore<JourneyState>(() => ({
  t: 0,
  vel: 0,
}));
