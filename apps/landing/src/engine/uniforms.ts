// Singleton uniform objects, shared by reference across every material that
// needs them. The SINGLE writer is the composer's priority-1 useFrame -- nothing
// else mutates these. Sharing one object per uniform means a material added
// later (the rip vertex shader, ink, crease passes) picks up the same live value
// with zero wiring. Kept as plain { value } holders so they drop straight into a
// three uniform slot.
//
//  uBoil       stepped hand-drawn boil clock: floor(elapsed * boilHz) / boilHz.
//  uRipP       Float32Array(9) -- per-page tear-front drive; uRipP[i] is the
//              PHASED tearP = smoothstep(0, 0.62, channels[i].exitP), NOT raw
//              exitP -- so the seam fully separates before the free piece is
//              thrown. Still a pure curve of the page's EXIT window (p in
//              [0.72,1] per ADR 0015), not its play window. Shader reads
//              uRipP[pageIndex]; the single writer is the composer.
//  uResolution drawing-buffer size, refreshed on resize.

import { Vector2 } from "three";

export const uBoil = { value: 0 };
export const uRipP = { value: new Float32Array(9) };
export const uResolution = { value: new Vector2(1, 1) };
