"use client";

// The one and only render driver. Lives inside the R3F <Canvas>. It owns the
// postprocessing EffectComposer (built once, RenderPass only in M1 -- the DOF /
// crease / grain passes are shader-engineer's later milestones) and the SOLE
// priority>0 useFrame in the app: a positive priority hands the render loop to
// us, so R3F stops auto-rendering and every frame flows through composer.render()
// here. Per frame it: (1) applies camera micro-parallax (pointer, time-damped --
// NOT scroll state) around the fixed down-look pose, (2) drives the free corner
// piece's rigid ballistic throw from channels[CORNER_TEAR_PAGE].exitP (pure
// f(exitP), so scrubbing back re-seats it), (3) steps uBoil, (4) uploads uRipP,
// then (5) renders. Reused module scratch + in-place mutation keep it
// allocation-free.

import { EffectComposer, RenderPass } from "postprocessing";
import { type RefObject,useEffect, useMemo } from "react";
import { type Group, MathUtils } from "three";
import { useFrame, useThree } from "@react-three/fiber";

import { channels } from "@/engine/channels";
import { CORNER_TEAR_PAGE } from "@/engine/pages";
import { resolveTier } from "@/engine/quality";
import { hudStats } from "@/engine/stats";
import { uBoil, uResolution, uRipP } from "@/engine/uniforms";

// Fixed camera pose: above and in front of the notebook, looking down ~25 deg at
// the page centred at the origin. Shared with the Canvas so the first frame is
// already framed. atan(y / z) ~= 25 deg.
export const CAMERA = { x: 0, y: 2.5, z: 5.4, fov: 35 } as const;

// Pointer micro-parallax: at ~6 units of range, 0.16 world units ~= 1.5 deg of
// apparent tilt. Damping is frame-rate independent (lambda per second).
const PARALLAX = 0.16;
const CAM_DAMP = 4;

// Rigid ballistic throw of the free corner piece, f(throwP). throwP 0 -> identity
// (piece at rest on the seam); throwP 1 -> tossed up-right toward camera with a
// tumble. y is a parabola (rise then fall) for a ballistic arc.
const THROW = {
  x: 1.3,
  up: 1.15,
  grav: 0.85,
  z: 1.4,
  rx: -1.1,
  ry: 0.7,
  rz: 1.8,
} as const;

// Tear/throw phasing (both pure f(exitP), so scrubbing back is exact): the
// shader tear-front (uRipP -> tearP) fully separates the seam by exitP 0.62
// BEFORE the piece is meaningfully thrown (throwP starts at 0.5), so the free
// group never rigid-translates while the seam is still attached. The 0.5-0.62
// overlap keeps the motion lively.
function smoothstep(e0: number, e1: number, x: number): number {
  const t = Math.min(1, Math.max(0, (x - e0) / (e1 - e0)));
  return t * t * (3 - 2 * t);
}

export function Composer({ freeRef }: { freeRef: RefObject<Group | null> }) {
  const gl = useThree((s) => s.gl);
  const scene = useThree((s) => s.scene);
  const camera = useThree((s) => s.camera);
  const tier = useMemo(() => resolveTier(), []);

  // Build the pipeline once. RenderPass draws the scene straight to screen (M1
  // has no effect passes yet). autoRenderToScreen stays off with renderToScreen
  // set explicitly on the last pass -- the durable safety default so a future
  // pass insertion can never leave the canvas black.
  const composer = useMemo(() => {
    const c = new EffectComposer(gl, { multisampling: 4 });
    const renderPass = new RenderPass(scene, camera);
    c.addPass(renderPass);
    c.autoRenderToScreen = false;
    renderPass.renderToScreen = true;
    return c;
  }, [gl, scene, camera]);

  // Keep composer buffers + uResolution matched to the viewport.
  useEffect(() => {
    const onResize = () => {
      composer.setSize(window.innerWidth, window.innerHeight);
      gl.getDrawingBufferSize(uResolution.value);
    };
    onResize();
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, [composer, gl]);

  // Release GPU resources when the Canvas unmounts.
  useEffect(() => () => composer.dispose(), [composer]);

  // Warm the page shader programs once, behind the loader, so the first scrub
  // into the tear has no compile hitch. Fire-and-forget (the material would
  // compile lazily on first render anyway); swallow rejection so a lost context
  // never surfaces as a console error.
  useEffect(() => {
    void gl.compileAsync(scene, camera).catch(() => {});
  }, [gl, scene, camera]);

  useFrame((state, delta) => {
    // (1) Camera micro-parallax around the fixed down-look pose. Time-damped
    // toward a pointer-driven target; never reads scroll state.
    const targetX = CAMERA.x + state.pointer.x * PARALLAX;
    const targetY = CAMERA.y + state.pointer.y * PARALLAX;
    camera.position.x = MathUtils.damp(camera.position.x, targetX, CAM_DAMP, delta);
    camera.position.y = MathUtils.damp(camera.position.y, targetY, CAM_DAMP, delta);
    camera.position.z = CAMERA.z;
    camera.lookAt(0, 0, 0);

    // (2) Free corner piece rigid throw, phased so it only flies once the seam
    // is (nearly) fully torn. Pure f(exitP) -> reversible.
    const tearCh = channels[CORNER_TEAR_PAGE];
    const exitP = tearCh ? tearCh.exitP : 0;
    const throwP = smoothstep(0.5, 1, exitP);
    const g = freeRef.current;
    if (g) {
      g.position.set(
        THROW.x * throwP,
        THROW.up * throwP - THROW.grav * throwP * throwP,
        THROW.z * throwP,
      );
      g.rotation.set(THROW.rx * throwP, THROW.ry * throwP, THROW.rz * throwP);
    }

    // (3) Stepped boil clock -- single writer.
    uBoil.value = Math.floor(state.clock.elapsedTime * tier.boilHz) / tier.boilHz;

    // (4) Per-page tear-front drive for the rip vertex shader: the PHASED tearP
    // (smoothstep of the page's exit progress), not raw exitP -- so the seam
    // separates and completes before the throw pulls the piece away.
    const rip = uRipP.value;
    let ri = 0;
    for (const ch of channels) {
      rip[ri] = smoothstep(0, 0.62, ch.exitP);
      ri += 1;
    }

    // (5) Render, then publish the frame's draw-call count for the dev HUD.
    composer.render(delta);
    hudStats.calls = gl.info.render.calls;
  }, 1);

  return null;
}
