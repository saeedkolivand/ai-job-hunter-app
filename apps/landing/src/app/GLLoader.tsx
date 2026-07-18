"use client";

import dynamic from "next/dynamic";
import {
  Component,
  type ReactNode,
  useCallback,
  useEffect,
  useState,
} from "react";

import { gateVerdict } from "@/engine/gate";
import { initLegacy } from "@/fallback/legacy";

// GL takeover boot. On mount it runs the one-shot capability probe (client-only);
// when the gate passes it mounts the WebGL Experience. The prerendered semantic
// layer stays visible and interactive until Experience reports onReady on its
// first painted frame -- only then is it hidden + made inert (never display:none,
// that would collapse scroll height). If the visitor activates the skip-link
// below, or the Experience throws during render, GLLoader unmounts GL, reverts
// the semantic layer, and boots the legacy engine itself -- LegacyBoot already
// stood down because the initial gate verdict was GL, so this is the only place
// left that can still start it.
//
// The real RIPBOOK Experience (M1) is dynamically imported with ssr:false: it
// touches window/WebGL, so it must never run during the static prerender. It
// calls onReady on its first painted frame.
const RipbookExperience = dynamic(
  () => import("@/experience/RipbookExperience"),
  { ssr: false },
);

// Render-phase error boundary around the Experience. A WebGL/init throw here is
// caught and turned into a legacy fallback instead of a blank page. (Event-time
// failures like context loss are a later hardening pass; M1 covers the render
// path.)
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
  const [ready, setReady] = useState(false);

  useEffect(() => {
    setMount(gateVerdict().gl);
  }, []);

  // Hide + inert the semantic root only once GL has actually painted a frame.
  // The cleanup reverts both -- Strict Mode's double-invoke, a future route
  // change, or falling back to legacy all flip `ready` back to false -- so the
  // semantic layer never gets stuck hidden.
  useEffect(() => {
    if (!ready) return;
    // .gl-active hides the legacy fixed chrome (see globals.css) that lives
    // outside #semantic-root; the root itself is hidden + inert.
    document.documentElement.classList.add("gl-active");
    const root = document.getElementById("semantic-root");
    if (root) {
      root.style.visibility = "hidden";
      root.setAttribute("inert", "");
    }
    return () => {
      document.documentElement.classList.remove("gl-active");
      if (root) {
        root.style.visibility = "";
        root.removeAttribute("inert");
      }
    };
  }, [ready]);

  const fallBackToLegacy = useCallback(() => {
    setReady(false);
    setMount(false);
    initLegacy();
  }, []);

  if (!mount) return null;

  return (
    <>
      {/* P1 a11y strategy: a one-keystroke escape hatch to the fully accessible
          semantic page for AT users on a capable desktop (deeper semantic twins
          land later). Rendered first so it is the first focusable element/Tab
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
        <RipbookExperience onReady={() => setReady(true)} />
      </ExperienceBoundary>
    </>
  );
}
