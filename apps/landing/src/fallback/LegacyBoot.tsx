"use client";

import { useEffect } from "react";

import { initLegacy } from "./legacy";

// Client boot shim. The static markup rendered by the semantic server components
// carries the original ids/classes/data-* hooks; the legacy interactivity
// (scroll engine, doodle pokes, sound toggle, konami) is bound here against
// those selectors by initLegacy(). initLegacy has its own double-init guard.
export default function LegacyBoot() {
  useEffect(() => {
    initLegacy();
  }, []);
  return null;
}
