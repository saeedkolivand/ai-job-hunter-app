"use client";

// The client orchestrator. It runs the Experience gate once at mount, owns the
// gl-live <-> slideshow transition state machine, freezes the svh scroll track,
// creates/destroys the scroll rig, and toggles the Semantic layer's visibility
// (visibility:hidden + inert when GL is live -- NEVER display:none, which would
// collapse the scroll height). The Semantic layer arrives as a prop so it stays
// a SERVER component (prerendered into the static export); this component only
// wraps it and flips its visibility.

import { type ReactNode, useCallback, useEffect, useRef } from "react";

import { A11yOverlay } from "@/a11y/A11yOverlay";
import { ChapterStepper } from "@/a11y/ChapterStepper";
import { MotionToggle } from "@/a11y/MotionToggle";
import { SkipLink } from "@/a11y/SkipLink";
import { GlCanvas } from "@/cast/GlCanvas";
import { Chrome } from "@/chrome/Chrome";

import { gatePasses, probeCapabilities, tierToQuality } from "./experience-gate";
import { nextMode } from "./motion-machine";
import { sceneById, sceneStartT } from "./scene-resolver";
import { createScrollRig, type ScrollRig } from "./scroll-rig";
import { frozenTrackHeightPx } from "./scroll-track";
import { playhead, useRig } from "./store";

export function Experience({ semantic }: { semantic: ReactNode }) {
  const mode = useRig((s) => s.mode);
  const tier = useRig((s) => s.tier);
  const motionReduced = useRig((s) => s.motionReduced);

  const semanticRef = useRef<HTMLDivElement>(null);
  const rigRef = useRef<ScrollRig | null>(null);
  const frozenVHRef = useRef<number | null>(null);
  const seekTargetRef = useRef<number | null>(null);
  const chapterRef = useRef<number>(0);

  // ---- Mount: freeze the viewport height, then run the gate once. -----------
  useEffect(() => {
    frozenVHRef.current = window.visualViewport?.height ?? window.innerHeight;
    let cancelled = false;
    void probeCapabilities().then((caps) => {
      if (cancelled) return;
      const store = useRig.getState();
      store.setMotionReduced(caps.reducedMotion);
      const pass = gatePasses(caps);
      if (pass) store.setTier(tierToQuality(caps.gpuTier));
      store.setMode(nextMode("pending", { type: "gate-resolved", pass }));
    });
    return () => {
      cancelled = true;
    };
  }, []);

  // ---- gl-live lifecycle: freeze the scroll track + own the rig. ------------
  useEffect(() => {
    if (mode !== "gl-live") return;

    const vh = frozenVHRef.current ?? window.innerHeight;
    const wrapper = semanticRef.current;
    // The Semantic layer is the scroll-height authority: give it the frozen
    // 3000 svh px height so the playhead has its full deterministic runway.
    if (wrapper) wrapper.style.minHeight = `${frozenTrackHeightPx(vh)}px`;

    const rig = createScrollRig();
    rigRef.current = rig;
    rig.start();

    // Reseed: an explicit chapter (restore-motion) wins; else a hash deep-link.
    let seekT = seekTargetRef.current;
    seekTargetRef.current = null;
    if (seekT == null) {
      const scene = sceneById(window.location.hash.replace("#", ""));
      if (scene) seekT = scene.lo;
    }
    if (seekT != null) rig.seek(seekT);

    return () => {
      rig.destroy();
      rigRef.current = null;
      if (wrapper) wrapper.style.minHeight = "";
    };
  }, [mode]);

  // ---- The one motion toggle handler = the transition state machine. --------
  const handleMotionToggle = useCallback(() => {
    const store = useRig.getState();
    if (!store.motionReduced) {
      // Reduce: freeze the film at the current chapter; the gl-live cleanup
      // tears the rig down and returns scroll to native. playhead.t stays frozen.
      chapterRef.current = playhead.scene;
      store.setMotionReduced(true);
      store.setMode(nextMode(store.mode, { type: "reduce-motion" }));
      return;
    }
    // Restore: re-run the FULL gate (the OS preference may have re-asserted).
    // motionReduced is set from the RESOLVED caps below, never optimistically --
    // on gate-fail (e.g. the OS preference re-asserted) the toggle must keep
    // showing "reduced" since the mode stays on the slideshow.
    void probeCapabilities().then((caps) => {
      const pass = gatePasses(caps);
      useRig.getState().setMotionReduced(caps.reducedMotion);
      if (pass) {
        useRig.getState().setTier(tierToQuality(caps.gpuTier));
        // The slideshow has no sub-chapter position: reseed at the chapter start.
        seekTargetRef.current = sceneStartT(chapterRef.current);
      }
      useRig
        .getState()
        .setMode(nextMode(useRig.getState().mode, { type: "restore-motion", gatePass: pass }));
    });
  }, []);

  const stepped = mode === "fallback" || mode === "slideshow";
  const glLive = mode === "gl-live";

  return (
    <>
      {/* The overlay only exists while GL is live (the semantic layer is
          hidden then); in every other mode the semantic layer IS the visible,
          already-accessible content, so mounting the overlay too would give AT
          users a duplicate menu and an aria-live region for a film that isn't
          playing. The skip-link target follows suit. */}
      <SkipLink href={glLive ? "#tv-content" : "#story-content"} />
      <MotionToggle reduced={motionReduced} onToggle={handleMotionToggle} />
      {glLive && <A11yOverlay />}
      {glLive && <GlCanvas tier={tier} />}
      {glLive && <Chrome />}
      {stepped && <ChapterStepper initialScene={chapterRef.current} />}
      <div
        ref={semanticRef}
        className={mode === "gl-live" ? "semantic-wrapper is-hidden" : "semantic-wrapper"}
        inert={mode === "gl-live"}
      >
        {semantic}
      </div>
    </>
  );
}
