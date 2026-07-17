// P1 placeholder -- back at the desk (world origin) to close the loop, with a
// framed accent so it reads distinct from the hero page. No fonts in P1.
// ponytail: flat MeshBasic stand-in (ceiling: no art yet).
export default function Finale() {
  return (
    <group position={[0, 0, 0]}>
      <mesh>
        <planeGeometry args={[10, 6]} />
        <meshBasicMaterial color="#f4ecdc" side={2} />
      </mesh>
      <mesh position={[0, 0, 0.1]}>
        <boxGeometry args={[7, 4, 0.1]} />
        <meshBasicMaterial color="#1c1812" />
      </mesh>
      <mesh position={[0, 0, 0.2]}>
        <boxGeometry args={[6, 3, 0.1]} />
        <meshBasicMaterial color="#f4ecdc" />
      </mesh>
    </group>
  );
}
