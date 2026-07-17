"use client";

// The GL takeover root (Wire phase). Mounts the full-canvas R3F <Canvas> that
// carries the 8-beat t-space sketchbook journey: SceneManager keeps the beats
// near the camera mounted, Composer owns the sole render loop (RenderPass + the
// always-on SketchbookEffect) and drives the camera pose from the scroll-driven
// global t. initScroll wires Lenis + ScrollTrigger to pump that t into
// journeyStore, and LoaderLift clears the boot overlay on the first painted GL
// frame.
//
// GLLoader only mounts this after the capability gate passes and after hiding
// (visibility:hidden + inert, never display:none) the semantic layer, which
// remains the scroll-height / SEO / a11y authority. The canvas wrapper is
// pointer-events:none for this phase -- there are no GL-side hit targets yet and
// the semantic layer is inert, so nothing should intercept pointer input.

import { useEffect } from "react";
import { Canvas, useFrame } from "@react-three/fiber";

import { resolveTier } from "@/engine/quality";
import { initScroll } from "@/engine/scroll";
import { Composer } from "@/post/composer";

import SceneManager from "./SceneManager";

// One-shot boot-overlay lift. Runs at priority 2 -- after Composer's priority-1
// composer.render() -- so #loader only starts fading once GL has actually
// painted its first frame (no flash of empty canvas). In GL mode the legacy
// engine, which normally clears the loader, stood down at the gate, so this is
// the only thing that lifts it. Idempotent via the module-guard-free ref-less
// early return once the class is applied.
function LoaderLift() {
  useFrame(() => {
    const loader = document.getElementById("loader");
    if (loader && !loader.classList.contains("gone")) {
      loader.classList.add("gone");
    }
  }, 2);
  return null;
}

export default function Experience() {
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
      <LoaderLift />
    </Canvas>
  );
}
