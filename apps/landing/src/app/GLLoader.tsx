"use client";

import { type ReactNode, useCallback, useEffect, useState } from "react";

import { gateVerdict } from "@/engine/gate";
import { initLegacy } from "@/fallback/legacy";

// GL takeover boot. On mount it runs the one-shot capability probe (client-only);
// when the gate passes it mounts the WebGL Experience. The prerendered semantic
// layer stays visible and interactive until Experience reports onReady on its
// first painted frame -- only then is it hidden + made inert (never display:none,
// that would collapse scroll height). If the visitor activates the skip-link
// below, GLLoader unmounts GL, reverts the semantic layer, and boots the legacy
// engine itself -- LegacyBoot already stood down because the initial gate verdict
// was GL, so this is the only place left that can still start it.
//
// RIPBOOK scaffold (PR0): the WebGL Experience is not built yet -- it arrives in
// M1. RipbookExperience is a placeholder that renders nothing and never reports
// ready, so on the GL-pass path the semantic layer simply stays visible. When the
// real Experience lands (M1) it replaces this stub with a dynamic ssr:false import
// that calls onReady on its first painted frame, and the boot restores the
// font-preload warm-up + a render-error boundary around it.
// TODO(ripbook M1): dynamic-import the real GL Experience here.
function RipbookExperience(_props: { onReady: () => void }): ReactNode {
  return null;
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
      <RipbookExperience onReady={() => setReady(true)} />
    </>
  );
}
