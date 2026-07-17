"use client";

// Mounts only the beats near the camera. Every frame it reads the scrub value
// straight off the vanilla store (journeyStore.getState().t -- no React
// selector in the hot path) and resolves which beat slice t sits in. It keeps
// the active beat and its two neighbours mounted and lets R3F unmount+dispose
// the rest, so the scene graph stays small as the camera rides the journey.

import { useState } from "react";
import { useFrame } from "@react-three/fiber";

import { BEATS } from "@/engine/beats";
import { journeyStore } from "@/engine/store";

function activeIndex(t: number): number {
  for (let i = 0; i < BEATS.length; i++) {
    const beat = BEATS[i];
    if (!beat) continue; // loop bound guarantees this index is always defined
    const [lo, hi] = beat.range;
    if (t < hi) return t < lo ? 0 : i;
  }
  return BEATS.length - 1;
}

export default function SceneManager() {
  const [active, setActive] = useState(0);

  // Default priority (0): observe the store, drive React only when the active
  // beat actually changes -- not a per-frame setState.
  useFrame(() => {
    const idx = activeIndex(journeyStore.getState().t);
    if (idx !== active) setActive(idx);
  });

  return (
    <>
      {BEATS.map((beat, i) => {
        if (Math.abs(i - active) > 1) return null;
        const Scene = beat.Scene;
        return <Scene key={beat.id} />;
      })}
    </>
  );
}
