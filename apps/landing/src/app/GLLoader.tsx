"use client";

import dynamic from "next/dynamic";
import { Component, type ReactNode, useCallback, useEffect, useState } from "react";

import { gateVerdict } from "@/engine/gate";
import { initLegacy } from "@/fallback/legacy";
import { preloadAllFonts } from "@/ink/text";

// GL takeover boot. On mount it runs the one-shot capability probe (client-only);
// when the gate passes it FIRST runs a font-preload phase (see below) and only
// then dynamically imports the WebGL Experience (ssr:false -- the static export
// ships zero GL JS on the legacy path). The prerendered semantic layer stays
// visible and interactive until Experience reports onReady on its first painted
// frame -- only then is it hidden + made inert (never display:none, that would
// collapse scroll height). If Experience throws post-mount, or the visitor
// activates the skip-link below, GLLoader unmounts it, reverts the semantic
// layer, and boots the legacy engine itself -- LegacyBoot already stood down
// because the initial gate verdict was GL, so this is the only place left that
// can still start it.
const Experience = dynamic(() => import("@/experience/Experience"), {
  ssr: false,
});

// Last-line fallback for the GL Experience: catches render-time errors in the
// Canvas tree (a bad shader compile, a missing resource) and hands control back
// to `onError` instead of leaving a blank canvas over an inert page.
class ExperienceBoundary extends Component<
  { onError: () => void; children: ReactNode },
  { failed: boolean }
> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  componentDidCatch() {
    this.props.onError();
  }

  render() {
    return this.state.failed ? null : this.props.children;
  }
}

export default function GLLoader() {
  // null = undecided (pre-probe); the probe touches window so it only runs
  // after mount. false and a later fallback both render nothing here --
  // LegacyBoot / initLegacy own the accessible page in that case.
  const [mount, setMount] = useState<boolean | null>(null);
  const [preloaded, setPreloaded] = useState(false);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    const gl = gateVerdict().gl;
    setMount(gl);
    if (!gl) return;
    // Boot-quality warm-up: fetch + atlas every GL font via troika's public
    // preloadFont() while #loader is still up (Experience/LoaderLift isn't
    // mounted yet), so scene text is ready on the first painted frame instead
    // of popping in. Not a crash fix -- the context-loss fix is the Suspense
    // boundary inside Experience's Canvas; see the comment there.
    let cancelled = false;
    preloadAllFonts().then(() => {
      if (!cancelled) setPreloaded(true);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  // Hide + inert the semantic root only once GL has actually painted a frame.
  // The cleanup reverts both -- Strict Mode's double-invoke, a future route
  // change, or falling back to legacy all flip `ready` back to false -- so the
  // semantic layer never gets stuck hidden.
  useEffect(() => {
    if (!ready) return;
    const root = document.getElementById("semantic-root");
    if (!root) return;
    root.style.visibility = "hidden";
    root.setAttribute("inert", "");
    return () => {
      root.style.visibility = "";
      root.removeAttribute("inert");
    };
  }, [ready]);

  const fallBackToLegacy = useCallback(() => {
    setReady(false);
    setPreloaded(false);
    setMount(false);
    initLegacy();
  }, []);

  if (!mount || !preloaded) return null;

  return (
    <>
      {/* P1 a11y strategy: a one-keystroke escape hatch to the fully accessible
          semantic page for AT users on a capable desktop (deeper semantic twins
          land in P6). Rendered first so it is the first focusable element/Tab
          stop whenever GL is mounted. */}
      <a
        href="#semantic-root"
        className="skip-gl"
        onClick={(e) => {
          e.preventDefault();
          fallBackToLegacy();
        }}
      >
        view accessible version
      </a>
      <ExperienceBoundary onError={fallBackToLegacy}>
        <Experience onReady={() => setReady(true)} />
      </ExperienceBoundary>
    </>
  );
}
