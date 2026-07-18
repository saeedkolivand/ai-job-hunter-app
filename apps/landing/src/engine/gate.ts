// Capability gate: the single source of truth for whether the WebGL experience
// takes over or the legacy prerendered DOM page runs. GLLoader (mounts GL) and
// LegacyBoot (binds the legacy scroll engine) both read gateVerdict(); the
// verdict is computed once, lazily, and cached so the two always agree.
//
// GL requires ALL of: WebGL2, a fine pointer, viewport wider than 900px, and
// no reduced-motion preference. Failing any one hands the page to legacy -- so
// reduced-motion users never reach the fried glitch/CA/dither (comfort
// contract). No window access at module scope: the probe runs on first call,
// which is always client-side (GLLoader/LegacyBoot invoke it from an effect).

export interface GateVerdict {
  gl: boolean;
  reason: string;
}

let cached: GateVerdict | null = null;

// Pre-launch guard: the GL experience is still being built (phases P4-P7,
// docs/adr/0014). Until the production flip, GL mounts only in dev or with an
// explicit ?gl=1 opt-in; every other visitor gets the legacy page exactly as
// shipped today. Remove this guard at the P7 flip.
function flipped(): boolean {
  if (process.env.NODE_ENV === "development") return true;
  return new URLSearchParams(window.location.search).has("gl");
}

function probe(): GateVerdict {
  if (typeof window === "undefined") return { gl: false, reason: "ssr" };
  if (!flipped()) return { gl: false, reason: "pre-launch" };

  let webgl2 = false;
  try {
    webgl2 = !!document.createElement("canvas").getContext("webgl2");
  } catch {
    // webgl2 stays false: a throwing/blocked getContext means no WebGL2.
  }
  if (!webgl2) return { gl: false, reason: "no-webgl2" };
  if (!window.matchMedia("(pointer: fine)").matches) {
    return { gl: false, reason: "no-fine-pointer" };
  }
  if (window.innerWidth <= 900) return { gl: false, reason: "viewport-narrow" };
  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
    return { gl: false, reason: "reduced-motion" };
  }
  return { gl: true, reason: "ok" };
}

export function gateVerdict(): GateVerdict {
  if (cached === null) cached = probe();
  return cached;
}
