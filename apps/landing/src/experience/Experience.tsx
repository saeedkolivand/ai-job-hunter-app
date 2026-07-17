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

import { Suspense, useCallback, useEffect, useRef, useState } from "react";
import { Canvas, useFrame, useThree } from "@react-three/fiber";

import { resolveTier } from "@/engine/quality";
import { initScroll } from "@/engine/scroll";
import FontsDebug from "@/ink/FontsDebug";
import { Composer } from "@/post/composer";

import SceneManager from "./SceneManager";

// Belt-and-braces WebGL2 context-loss recovery (P2 gate finding). preventDefault
// on "webglcontextlost" is required for the browser to even attempt automatic
// restoration -- without it "webglcontextrestored" never fires. If restoration
// doesn't happen within ~3s, treat it as permanent and call onPermanentLoss so
// the caller can throw during render: that is how a plain DOM/GL event reaches
// GLLoader's ExperienceBoundary (React error boundaries only catch render-phase
// throws, not event-listener callbacks).
function ContextWatchdog({ onPermanentLoss }: { onPermanentLoss: () => void }) {
  const gl = useThree((s) => s.gl);
  useEffect(() => {
    const canvas = gl.domElement;
    let timer: number | undefined;
    const onLost = (e: Event) => {
      e.preventDefault();
      timer = window.setTimeout(onPermanentLoss, 3000);
    };
    const onRestored = () => {
      if (timer !== undefined) window.clearTimeout(timer);
    };
    canvas.addEventListener("webglcontextlost", onLost);
    canvas.addEventListener("webglcontextrestored", onRestored);
    return () => {
      canvas.removeEventListener("webglcontextlost", onLost);
      canvas.removeEventListener("webglcontextrestored", onRestored);
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [gl, onPermanentLoss]);
  return null;
}

// Debug-only font smoke test: ?fonts=1 renders a static grid of every
// self-hosted family (see ink/FontsDebug) instead of the journey, so the gate
// audit can screenshot all typefaces at once. Query param only -- no route,
// never reached in a normal build.
function fontsDebugMode(): boolean {
  if (typeof window === "undefined") return false;
  return new URLSearchParams(window.location.search).has("fonts");
}

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
  const fontsMode = fontsDebugMode();
  const [permanentLoss, setPermanentLoss] = useState(false);
  const markPermanentLoss = useCallback(() => setPermanentLoss(true), []);

  // Scroll spine: mounted once, client-only. initScroll returns its own cleanup
  // (kills the ScrollTrigger, destroys Lenis, detaches the ticker/listeners), so
  // a StrictMode double-mount tears the first graph down cleanly. Skipped in the
  // font-debug view, which has no journey to scroll.
  useEffect(() => {
    if (fontsMode) return;
    return initScroll();
  }, [fontsMode]);

  // Re-throw a permanent context loss during render so GLLoader's
  // ExperienceBoundary catches it and falls back to legacy, same path as any
  // other GL failure.
  if (permanentLoss) {
    throw new Error("WebGL2 context lost and did not restore");
  }

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
      <ContextWatchdog onPermanentLoss={markPermanentLoss} />
      {/* This Suspense boundary is load-bearing, not cosmetic (P2 gate root
          cause). drei <Text> suspends on first render of each font while
          troika typesets it. Without a boundary INSIDE the Canvas, R3F's
          CanvasImpl re-throws that suspension into the parent React tree; the
          parent boundary (next/dynamic's) then hides the Canvas subtree,
          React runs the hidden tree's effect cleanups, and R3F's cleanup
          schedules gl.forceContextLoss() 500ms later WITHOUT cancelling it
          when the tree is revealed and the root reused -- permanently killing
          the live WebGL2 context ~1s after boot. Catching the suspension here
          keeps it out of CanvasImpl entirely. Traced on r3f 9.6.1:
          CanvasImpl's "if (block) throw block" + unmountComponentAtNode's
          delayed forceContextLoss.
          Scope rule: ONLY the suspending scene content (and LoaderLift, which
          holds no GPU resources and must wait for that content's first painted
          frame) goes inside. Composer must stay OUTSIDE: each re-suspension
          hides the boundary's children and runs their effect cleanups, and
          Composer's cleanup disposes its memoized EffectComposer -- which the
          reveal never rebuilds (useMemo state survives hiding), leaving a
          permanently dead render loop (verified: 0 draw calls/frame).
          ContextWatchdog also stays outside so it keeps watching while
          content suspends. */}
      {!fontsMode && <Composer />}
      <Suspense fallback={null}>
        {fontsMode ? (
          <FontsDebug onReady={onReady} />
        ) : (
          <>
            <SceneManager />
            <LoaderLift onReady={onReady} />
          </>
        )}
      </Suspense>
    </Canvas>
  );
}
