"use client";

// The chapter-stepped presentation for the fallback + reduced-motion/slideshow
// paths: a frozen camera equivalent. Prev/Next step through the 9 scenes,
// scrolling the (now visible) Semantic layer sections into view and announcing
// the act. Seeded at the chapter containing the frozen playhead when arriving
// from a live -> slideshow transition.

import { useState } from "react";

import { SCENES } from "@/engine/scene-resolver";

export function ChapterStepper({ initialScene = 0 }: { initialScene?: number }) {
  const [i, setI] = useState(() =>
    Math.max(0, Math.min(SCENES.length - 1, initialScene)),
  );
  const scene = SCENES[i];

  function go(next: number): void {
    const clamped = Math.max(0, Math.min(SCENES.length - 1, next));
    setI(clamped);
    const target = SCENES[clamped];
    if (target && typeof document !== "undefined") {
      // "auto" (instant), not "smooth" -- this stepper IS the fallback +
      // reduced-motion/slideshow path, so the users who reach it are often
      // here specifically BECAUSE they asked for less motion.
      document.getElementById(target.id)?.scrollIntoView({ behavior: "auto", block: "start" });
    }
  }

  return (
    <div className="chapter-stepper" role="group" aria-label="Chapters">
      <button type="button" onClick={() => go(i - 1)} disabled={i === 0}>
        Prev
      </button>
      <span aria-live="polite" aria-atomic="true">
        {scene?.act ?? ""} ({i + 1}/{SCENES.length})
      </span>
      <button type="button" onClick={() => go(i + 1)} disabled={i === SCENES.length - 1}>
        Next
      </button>
    </div>
  );
}
