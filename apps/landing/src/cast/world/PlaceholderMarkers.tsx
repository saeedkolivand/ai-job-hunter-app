"use client";

// The M1 per-scene boundary markers, kept for scenes NOT yet built (3-8). M2
// delivers the canyon only; CanyonWorld shows this static debug column instead
// while the playhead is below the canyon so those ranges stay visibly wired. No
// useFrame here -- CanyonWorld owns the camera + visibility gating.

import { SCENES } from "@/engine/scene-resolver";

import { WORLD_HEIGHT } from "./canyon-layout";

// One debug tint per scene (author-side data, not a shader).
const SCENE_HEX = [
  "#10233f", // cold-open  -- monitor blue
  "#1a1740", // canyon     -- indigo
  "#12333a", // surface    -- teal
  "#0c2230", // deep       -- deep blue-green
  "#050608", // blackout   -- near black
  "#3a2408", // catch      -- amber
  "#123028", // ascent     -- green-blue
  "#c9922e", // dawn       -- gold
  "#c0392b", // finale     -- send red
];

const MARKERS = SCENES.map((sc, i) => ({
  id: sc.id,
  x: i % 2 === 0 ? -1.7 : 1.7,
  y: -sc.lo * WORLD_HEIGHT,
  hex: SCENE_HEX[i] ?? "#808080",
}));

export function PlaceholderMarkers() {
  return (
    <>
      {/* vertical guide spanning the whole column */}
      <mesh position={[0, -WORLD_HEIGHT / 2, -2]}>
        <boxGeometry args={[0.06, WORLD_HEIGHT, 0.06]} />
        <meshBasicMaterial color="#46587a" />
      </mesh>

      {/* per-scene boundary markers */}
      {MARKERS.map((m) => (
        <mesh key={m.id} position={[m.x, m.y, 0]}>
          <boxGeometry args={[1.5, 0.32, 1.5]} />
          <meshStandardMaterial color={m.hex} emissive={m.hex} emissiveIntensity={0.45} />
        </mesh>
      ))}
    </>
  );
}
