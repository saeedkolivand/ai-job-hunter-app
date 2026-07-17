// P1 placeholder -- the wall: a page carrying a 2x2 grid of dark cards at
// x=26. No fonts in P1.
// ponytail: flat MeshBasic stand-in (ceiling: no art yet).
export default function Testimonials() {
  const cells: [number, number][] = [
    [-2.4, 1.6],
    [2.4, 1.6],
    [-2.4, -1.6],
    [2.4, -1.6],
  ];
  return (
    <group position={[26, 17, 0]}>
      <mesh>
        <planeGeometry args={[10, 8]} />
        <meshBasicMaterial color="#f4ecdc" side={2} />
      </mesh>
      {cells.map(([x, y]) => (
        <mesh key={`${x},${y}`} position={[x, y, 0.1]}>
          <boxGeometry args={[3.4, 2.4, 0.1]} />
          <meshBasicMaterial color="#1c1812" />
        </mesh>
      ))}
    </group>
  );
}
