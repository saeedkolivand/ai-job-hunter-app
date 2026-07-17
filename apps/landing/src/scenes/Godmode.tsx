// P1 placeholder -- the high point: an inverted page (dark ground, light
// accent) up at y=+22 to signal the tonal flip after the fried bottom. No fonts
// in P1.
// ponytail: flat MeshBasic stand-in (ceiling: no art yet).
export default function Godmode() {
  return (
    <group position={[0, 22, 0]}>
      <mesh>
        <planeGeometry args={[12, 8]} />
        <meshBasicMaterial color="#1c1812" side={2} />
      </mesh>
      <mesh position={[0, 0, 0.1]}>
        <boxGeometry args={[7, 1, 0.1]} />
        <meshBasicMaterial color="#f4ecdc" />
      </mesh>
    </group>
  );
}
