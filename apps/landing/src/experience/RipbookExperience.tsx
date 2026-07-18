"use client";

// The GL takeover root (M2). Mounts the full-canvas R3F <Canvas> carrying the
// RIPBOOK notebook on a dark desk: the notebook shell (Book), the one live
// corner-tear page (PageMesh), the desk + dressing (Desk), the ink test prop
// (InkDummy), the composer that owns the sole render loop + camera + uniforms,
// and the scroll spine (initScroll).
//
// Boot sequencing (Stage): the bake harness runs FIRST behind the loader
// (bakeAll fills the `bakes` singletons), then the scene mounts with those baked
// textures, then the shader programs are warmed (compileAsync), and only THEN is
// onReady reported -- the loader stays up the whole time. The M1 LoaderLift is
// folded into Stage's priority-2 useFrame so the overlay lifts on the first
// painted frame after warmup. The canvas is pointer-events:none for M2 -- there
// are no GL-side hit targets yet.

import { useEffect, useRef, useState } from "react";
import type { Group } from "three";
import { Canvas, useFrame, useThree } from "@react-three/fiber";

import { bakeAll } from "@/bake/bake";
import Book from "@/book/Book";
import PageMesh from "@/book/PageMesh";
import Hud from "@/debug/Hud";
import Desk from "@/desk/Desk";
import InkDummy from "@/desk/InkDummy";
import { initScroll } from "@/engine/scroll";

import { CAMERA, Composer } from "./Composer";

// Everything inside the Canvas. Owns the refs the composer drives (free corner
// piece + cover hinge), the bake -> compile -> ready sequence, and the loader
// lift. The composer stays mounted throughout (it owns the render loop); the rest
// of the scene mounts only once the bakes are in, so no material ever samples a
// null bake texture.
function Stage({
  onReady,
  onError,
}: {
  onReady: () => void;
  onError: () => void;
}) {
  const gl = useThree((s) => s.gl);
  const scene = useThree((s) => s.scene);
  const camera = useThree((s) => s.camera);

  const freeRef = useRef<Group>(null);
  const coverRef = useRef<Group>(null);

  const [baked, setBaked] = useState(false);
  const ready = useRef(false);
  const lifted = useRef(false);

  // (1) Bakes first, behind the loader. bakeAll is synchronous (RT renders) and
  // idempotent, so a StrictMode double-mount never re-bakes. On success unlock the
  // scene mount; on failure call onError (a PLAIN function -> GLLoader's
  // fallBackToLegacy). We deliberately do NOT re-throw to a React boundary: the
  // bake runs inside the R3F reconciler root, whose errors never cross into the
  // DOM tree where GLLoader's ExperienceBoundary lives -- a throw here would just
  // hang the loader. A direct callback is boundary-independent.
  useEffect(() => {
    try {
      bakeAll(gl);
      setBaked(true);
    } catch (err) {
      console.error("[ripbook] bake failed; falling back to legacy", err);
      onError();
    }
  }, [gl, onError]);

  // (2) Once the scene has mounted with its baked textures, warm the shader
  // programs so the first scrub has no compile hitch, then flag ready. A failed
  // warmup means the GL path is broken -> fall back to legacy (via onError)
  // rather than lifting the loader onto a possibly-black canvas.
  useEffect(() => {
    if (!baked) return;
    let cancelled = false;
    gl.compileAsync(scene, camera)
      .then(() => {
        if (!cancelled) ready.current = true;
      })
      .catch((err) => {
        if (cancelled) return;
        console.error("[ripbook] shader warmup failed; falling back", err);
        onError();
      });
    return () => {
      cancelled = true;
    };
  }, [baked, gl, scene, camera, onError]);

  // (3) Lift the boot overlay + report ready on the first painted frame after
  // warmup. Priority 2 runs after the composer's priority-1 render, so onReady
  // only fires once GL has actually painted the full scene.
  useFrame(() => {
    if (lifted.current || !ready.current) return;
    document.getElementById("loader")?.classList.add("gone");
    lifted.current = true;
    onReady();
  }, 2);

  return (
    <>
      {/* One scene directional light (+ ambient) for the placeholder standard
          materials on the book + desk; the paper + ink materials carry their own
          light for now. Direction matches the paper shader's upper-front key. */}
      <ambientLight intensity={0.55} />
      <directionalLight position={[-3, 5, 4]} intensity={1.5} />

      <Composer freeRef={freeRef} coverRef={coverRef} />

      {baked && (
        <>
          <Book coverRef={coverRef} />
          <PageMesh freeRef={freeRef} />
          <Desk />
          <InkDummy />
        </>
      )}
    </>
  );
}

export default function RipbookExperience({
  onReady,
  onError,
}: {
  onReady: () => void;
  onError: () => void;
}) {
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
        <Stage onReady={onReady} onError={onError} />
      </Canvas>
      <Hud />
    </>
  );
}
