// P1 placeholder -- a slightly tilted paper page sitting down/forward of the
// desk. Tilt telegraphs the slump before the plunge. No fonts in P1.
// ponytail: flat MeshBasic stand-in (ceiling: no art yet).
export default function Slump() {
  return (
    <group position={[1, -4, 0]} rotation={[0, 0, -0.14]}>
      <mesh>
        <planeGeometry args={[9, 6]} />
        <meshBasicMaterial color="#ece2cd" side={2} />
      </mesh>
      <mesh position={[0, -1, 0.1]}>
        <boxGeometry args={[4.5, 0.6, 0.1]} />
        <meshBasicMaterial color="#1c1812" />
      </mesh>
    </group>
  );
}
