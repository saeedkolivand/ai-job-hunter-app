// P1 placeholder -- paper page + a single dark title bar. Sits at the desk
// (world origin), the hero waypoint's look target. No fonts in P1 (that is P2).
// ponytail: flat MeshBasic paper stand-in (ceiling: no ink/boil/text yet).
// Upgrade path: P2 replaces the accent geometry with drei Text + Line2 strokes.
export default function Hero() {
  return (
    <group position={[0, 0, 0]}>
      <mesh>
        <planeGeometry args={[10, 6]} />
        <meshBasicMaterial color="#f4ecdc" side={2} />
      </mesh>
      <mesh position={[0, 1, 0.1]}>
        <boxGeometry args={[6, 0.8, 0.1]} />
        <meshBasicMaterial color="#1c1812" />
      </mesh>
    </group>
  );
}
