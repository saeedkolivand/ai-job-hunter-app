"use client";

import dynamic from "next/dynamic";
import { useEffect, useState } from "react";

import { gateVerdict } from "@/engine/gate";

// GL takeover boot. On mount it runs the one-shot capability probe (client-only)
// and, only when the gate passes, dynamically imports the WebGL Experience
// (ssr:false -- the static export ships zero GL JS on the legacy path) and hands
// the page to GL: the prerendered semantic layer is hidden + made inert so it
// stays the scroll-height / SEO / a11y authority without stealing paint or
// focus. It is NEVER display:none'd (that would collapse scroll height and break
// the journey). LegacyBoot reads the same cached verdict and stands down.
const Experience = dynamic(() => import("@/experience/Experience"), {
  ssr: false,
});

export default function GLLoader() {
  // null = undecided (pre-probe). The probe touches window, so it only runs
  // after mount; nothing renders until the verdict is in.
  const [mount, setMount] = useState<boolean | null>(null);

  useEffect(() => {
    if (!gateVerdict().gl) {
      setMount(false);
      return;
    }
    const root = document.getElementById("semantic-root");
    if (root) {
      root.style.visibility = "hidden";
      root.setAttribute("inert", "");
    }
    setMount(true);
  }, []);

  return mount ? <Experience /> : null;
}
