// P1 placeholder -- the bottom of the shaft: a page scattered with jittered
// dark blocks, standing in for the deep-fried glitch beat (Pass B lives here in
// a later phase). No fonts in P1.
// ponytail: flat MeshBasic stand-in (ceiling: no glitch/CA/dither yet).
export default function Fried() {
  return (
    <group position={[0, -43, 0]}>
      <mesh>
        <planeGeometry args={[10, 8]} />
        <meshBasicMaterial color="#f4ecdc" side={2} />
      </mesh>
      <mesh position={[-2.5, 1.2, 0.1]} rotation={[0, 0, 0.2]}>
        <boxGeometry args={[2.4, 1.2, 0.1]} />
        <meshBasicMaterial color="#1c1812" />
      </mesh>
      <mesh position={[2, -0.6, 0.1]} rotation={[0, 0, -0.3]}>
        <boxGeometry args={[1.8, 1.6, 0.1]} />
        <meshBasicMaterial color="#1c1812" />
      </mesh>
      <mesh position={[0.4, 2, 0.1]} rotation={[0, 0, 0.5]}>
        <boxGeometry args={[1.2, 0.8, 0.1]} />
        <meshBasicMaterial color="#1c1812" />
      </mesh>
    </group>
  );
}
