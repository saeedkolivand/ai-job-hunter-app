// P1 placeholder -- a tall vertical page marking the plunge shaft. Elongated
// so the drop reads as motion as the camera falls past it. No fonts in P1.
// ponytail: flat MeshBasic stand-in (ceiling: no art yet).
export default function Descent() {
  return (
    <group position={[2, -24, 0]}>
      <mesh>
        <planeGeometry args={[6, 16]} />
        <meshBasicMaterial color="#f4ecdc" side={2} />
      </mesh>
      <mesh position={[0, 0, 0.1]}>
        <boxGeometry args={[0.6, 12, 0.1]} />
        <meshBasicMaterial color="#1c1812" />
      </mesh>
    </group>
  );
}
