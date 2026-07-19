"use client";

// A few hero desk objects tumbling alongside the fall (mug, chair, sticky-note
// cluster) -- placeholder-grade procedural silhouettes; art comes later. Each is
// a small group whose whole transform is a pure f(t) parallax written every frame
// from canyon-layout.writeDeskProp (no per-frame allocation -- a single reused
// scratch object). Visible only while the canyon framing is active. Animation
// useFrame stays at the DEFAULT priority (0).

import { useMemo, useRef } from "react";
import type { Group } from "three";
import { useFrame } from "@react-three/fiber";

import { playhead } from "@/engine/store";

import { DESK_PROPS, type DeskPropState, writeDeskProp } from "./canyon-layout";

function Mug() {
  return (
    <group>
      <mesh>
        <cylinderGeometry args={[0.34, 0.3, 0.8, 16, 1, true]} />
        <meshStandardMaterial color="#e8e4dc" roughness={0.6} metalness={0.05} />
      </mesh>
      <mesh position={[0.42, 0, 0]} rotation={[Math.PI / 2, 0, 0]}>
        <torusGeometry args={[0.2, 0.06, 8, 16]} />
        <meshStandardMaterial color="#e8e4dc" roughness={0.6} metalness={0.05} />
      </mesh>
    </group>
  );
}

function Chair() {
  return (
    <group>
      <mesh position={[0, 0, 0]}>
        <boxGeometry args={[1.2, 0.16, 1.2]} />
        <meshStandardMaterial color="#2c2f36" roughness={0.8} metalness={0.1} />
      </mesh>
      <mesh position={[0, 0.9, -0.52]}>
        <boxGeometry args={[1.2, 1.6, 0.16]} />
        <meshStandardMaterial color="#2c2f36" roughness={0.8} metalness={0.1} />
      </mesh>
    </group>
  );
}

function StickyNotes() {
  return (
    <group>
      <mesh position={[0, 0, 0]} rotation={[0, 0, 0.2]}>
        <boxGeometry args={[0.6, 0.6, 0.02]} />
        <meshStandardMaterial color="#f3d35b" roughness={0.9} />
      </mesh>
      <mesh position={[0.3, -0.2, 0.1]} rotation={[0, 0, -0.35]}>
        <boxGeometry args={[0.55, 0.55, 0.02]} />
        <meshStandardMaterial color="#f28ca6" roughness={0.9} />
      </mesh>
      <mesh position={[-0.25, 0.28, -0.08]} rotation={[0, 0, 0.5]}>
        <boxGeometry args={[0.5, 0.5, 0.02]} />
        <meshStandardMaterial color="#9fd4a3" roughness={0.9} />
      </mesh>
    </group>
  );
}

const PROP_NODES = [<Mug key="mug" />, <Chair key="chair" />, <StickyNotes key="sticky" />];

export function DeskObjects() {
  const groupsRef = useRef<(Group | null)[]>([]);
  // One reused scratch object -> writeDeskProp fills it in place each frame.
  const scratch = useMemo<DeskPropState>(
    () => ({ x: 0, y: 0, z: 0, rx: 0, ry: 0, rz: 0, visible: false }),
    [],
  );

  useFrame(() => {
    const t = playhead.t;
    for (let i = 0; i < DESK_PROPS.length; i++) {
      const g = groupsRef.current[i];
      if (!g) continue;
      writeDeskProp(i, t, scratch);
      g.visible = scratch.visible;
      if (!scratch.visible) continue;
      g.position.set(scratch.x, scratch.y, scratch.z);
      g.rotation.set(scratch.rx, scratch.ry, scratch.rz);
    }
  });

  return (
    <>
      {PROP_NODES.map((node, i) => (
        <group
          key={i}
          visible={false}
          ref={(el) => {
            groupsRef.current[i] = el;
          }}
        >
          {node}
        </group>
      ))}
    </>
  );
}
