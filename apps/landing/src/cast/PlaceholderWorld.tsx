"use client";

// M1 placeholder world -- NO final art, NO post chain. It exists only to prove
// the scroll rig: a camera translates straight down a debug column with a
// per-scene boundary marker, reading the playhead from the store singleton every
// frame (never a hook selector). Scrolling up rewinds it. This animation
// useFrame runs at the DEFAULT priority (0); no numeric priority anywhere until
// the composer lands (a numeric priority would disable R3F auto-render).

import { useMemo, useRef } from "react";
import { Color } from "three";
import { useFrame } from "@react-three/fiber";

import { SCENES } from "@/engine/scene-resolver";
import { playhead } from "@/engine/store";

const WORLD_HEIGHT = 240; // world units, top (t=0) to bottom (t=1)

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

export function PlaceholderWorld() {
  const colors = useMemo(() => SCENE_HEX.map((h) => new Color(h)), []);
  const bg = useRef(new Color(SCENE_HEX[0]));

  useFrame((state) => {
    const t = playhead.t;
    // Pure f(t): descend the column. Rewinds exactly when t decreases.
    state.camera.position.y = -t * WORLD_HEIGHT;

    // Background crossfades between the active scene tint and the next, driven by
    // the rig-computed sceneProgress -- proves scene + sub-progress are wired.
    const from = colors[playhead.scene];
    const to = colors[Math.min(playhead.scene + 1, colors.length - 1)];
    if (from && to) {
      bg.current.copy(from).lerp(to, playhead.sceneProgress);
      state.scene.background = bg.current;
    }
  });

  return (
    <>
      <ambientLight intensity={0.9} />
      <directionalLight position={[3, 4, 5]} intensity={0.6} />

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
