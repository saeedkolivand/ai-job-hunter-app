// The runtime motion-toggle transition, as one explicit state machine rather
// than two independent gate evaluations that can race. The Experience gate only
// blocks GL at MOUNT; the in-page motion toggle can flip while GL is live, so
// the live transition lives here. This module is pure (no DOM) so the transition
// table is unit-testable; the Experience component wires the side effects
// (destroy/create the rig, hide/show the canvas, seek the playhead) to it.

import type { RigMode } from "./store";

export type MotionAction =
  | { type: "gate-resolved"; pass: boolean } // initial capability probe finished
  | { type: "reduce-motion" } // user toggled motion OFF while GL was live
  | { type: "restore-motion"; gatePass: boolean }; // user toggled motion back ON

// Pure next-mode transition. `pending` resolves to gl-live or fallback from the
// initial gate; a live -> slideshow flip freezes the film; a slideshow ->
// gl-live flip only succeeds if the full gate still passes (e.g. the OS
// reduced-motion preference has not re-asserted), otherwise it stays on the
// slideshow.
export function nextMode(current: RigMode, action: MotionAction): RigMode {
  switch (action.type) {
    case "gate-resolved":
      return action.pass ? "gl-live" : "fallback";
    case "reduce-motion":
      return current === "gl-live" ? "slideshow" : current;
    case "restore-motion":
      if (current !== "slideshow") return current;
      return action.gatePass ? "gl-live" : "slideshow";
    default:
      return current;
  }
}
