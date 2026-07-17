"use client";

// The GL takeover root (Wire phase). Mounts the full-canvas R3F <Canvas> that
// carries the 8-beat t-space sketchbook journey: SceneManager keeps the beats
// near the camera mounted, Composer owns the sole render loop (RenderPass + the
// always-on SketchbookEffect) and drives the camera pose from the scroll-driven
// global t. initScroll wires Lenis + ScrollTrigger to pump that t into
// journeyStore, and LoaderLift clears the boot overlay on the first painted GL
// frame.
//
// GLLoader mounts this after the capability gate passes, but only hides
// (visibility:hidden + inert, never display:none) the semantic layer once
// this component reports onReady on its first painted frame -- see GLLoader
// for the full choreography (error boundary + skip-link fall back to legacy).
// The canvas wrapper is pointer-events:none for this phase -- there are no
// GL-side hit targets yet, so nothing should intercept pointer input.

import { useEffect, useRef } from "react";
import { Canvas, useFrame } from "@react-three/fiber";

import { resolveTier } from "@/engine/quality";
import { initScroll } from "@/engine/scroll";
import { Composer } from "@/post/composer";

import SceneManager from "./SceneManager";

// One-shot boot-overlay lift + ready signal. Runs at priority 2 -- after
// Composer's priority-1 composer.render() -- so #loader only starts fading,
// and onReady only fires, once GL has actually painted its first frame (no
// flash of empty canvas). In GL mode the legacy engine, which normally clears
// the loader, stood down at the gate, so this is the only thing that lifts
// it. Guarded by a `done` ref, checked first, so the per-frame DOM lookup
// only ever runs once per mount instead of every frame for the whole
// session. If #loader is missing entirely, still mark done and fire onReady
// -- otherwise the semantic root would never hide.
function LoaderLift({ onReady }: { onReady: () => void }) {
  const done = useRef(false);
  useFrame(() => {
    if (done.current) return;
    document.getElementById("loader")?.classList.add("gone");
    done.current = true;
    onReady();
  }, 2);
  return null;
}

export default function Experience({ onReady }: { onReady: () => void }) {
  const tier = resolveTier();

  // Scroll spine: mounted once, client-only. initScroll returns its own cleanup
  // (kills the ScrollTrigger, destroys Lenis, detaches the ticker/listeners), so
  // a StrictMode double-mount tears the first graph down cleanly.
  useEffect(() => initScroll(), []);

  return (
    <Canvas
      frameloop="always"
      dpr={tier.dpr}
      gl={{ antialias: false }}
      style={{
        position: "fixed",
        top: 0,
        right: 0,
        bottom: 0,
        left: 0,
        zIndex: 1,
        pointerEvents: "none",
      }}
    >
      <SceneManager />
      <Composer />
      <LoaderLift onReady={onReady} />
    </Canvas>
  );
}
