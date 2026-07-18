// RIPBOOK scroll state: the single vanilla zustand store the whole GL engine
// reads per-frame. scroll.ts is the ONLY writer (via ScrollTrigger.onUpdate);
// every consumer (composer, page transforms, HUD) reads ripbookStore.getState()
// inside its useFrame -- never a React selector in a hot path, which would
// re-render the tree on every scroll tick. Three fields only: t (global scroll
// progress in [0,1], the t-space authority), vel (Lenis scroll velocity), and
// activePage (the 0..8 page index t currently sits in).

import { createStore } from "zustand/vanilla";

export interface RipbookState {
  t: number;
  vel: number;
  activePage: number;
}

export const ripbookStore = createStore<RipbookState>(() => ({
  t: 0,
  vel: 0,
  activePage: 0,
}));

// Single scroll-write setter. Called only from scroll.ts's ScrollTrigger
// onUpdate (a scroll event, not the render loop), so the small partial-object
// churn here never touches the per-frame path.
export function setScroll(t: number, vel: number, activePage: number): void {
  ripbookStore.setState({ t, vel, activePage });
}
