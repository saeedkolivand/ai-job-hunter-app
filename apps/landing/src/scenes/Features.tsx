// P1 placeholder -- the corridor: a wide page with a row of dark blocks the
// camera tracks along at x=22. No fonts in P1.
// ponytail: flat MeshBasic stand-in (ceiling: no art yet).
export default function Features() {
  return (
    <group position={[22, 19, 0]}>
      <mesh>
        <planeGeometry args={[14, 6]} />
        <meshBasicMaterial color="#f4ecdc" side={2} />
      </mesh>
      {[-4.5, 0, 4.5].map((x) => (
        <mesh key={x} position={[x, 0, 0.1]}>
          <boxGeometry args={[2.6, 3, 0.1]} />
          <meshBasicMaterial color="#1c1812" />
        </mesh>
      ))}
    </group>
  );
}
