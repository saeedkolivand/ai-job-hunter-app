"use client";

// The GL takeover root (M1). Mounts the full-canvas R3F <Canvas> that carries
// the RIPBOOK notebook on a dark desk: one pre-split kraft page (PageMesh), the
// composer that owns the sole render loop + camera + uniforms, and the scroll
// spine (initScroll) that pumps the global t / channels. LoaderLift clears the
// boot overlay on the first painted GL frame and reports onReady; GLLoader then
// hides the semantic layer. The dev HUD is a DOM sibling (never over hit
// targets, pointer-events:none). The canvas is pointer-events:none for M1 --
// there are no GL-side hit targets yet.

import { useEffect, useRef } from "react";
import type { Group } from "three";
import { Canvas, useFrame } from "@react-three/fiber";

import PageMesh from "@/book/PageMesh";
import Hud from "@/debug/Hud";
import { initScroll } from "@/engine/scroll";

import { CAMERA, Composer } from "./Composer";

// One-shot boot-overlay lift + ready signal. Runs at priority 2 -- after
// Composer's priority-1 composer.render() -- so #loader only fades, and onReady
// only fires, once GL has actually painted its first frame (no flash of empty
// canvas). In GL mode the legacy engine (which normally clears the loader) stood
// down at the gate, so this is the only thing that lifts it. Guarded by a ref so
// the per-frame DOM lookup runs once. If #loader is missing, still fire onReady
// or the semantic root would never hide.
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

export default function RipbookExperience({ onReady }: { onReady: () => void }) {
  const freeRef = useRef<Group>(null);

  // Scroll spine: mounted once, client-only. initScroll returns its own cleanup
  // (kills the ScrollTrigger + master timeline, destroys Lenis, detaches the
  // ticker/listeners), so a StrictMode double-mount tears the first graph down
  // cleanly.
  useEffect(() => initScroll(), []);

  return (
    <>
      <Canvas
        frameloop="always"
        dpr={[1, 2]}
        gl={{ antialias: true }}
        camera={{
          position: [CAMERA.x, CAMERA.y, CAMERA.z],
          fov: CAMERA.fov,
          near: 0.1,
          far: 100,
        }}
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
        {/* Dark kraft desk. */}
        <color attach="background" args={["#141013"]} />
        <Composer freeRef={freeRef} />
        <PageMesh freeRef={freeRef} />
        <LoaderLift onReady={onReady} />
      </Canvas>
      <Hud />
    </>
  );
}
