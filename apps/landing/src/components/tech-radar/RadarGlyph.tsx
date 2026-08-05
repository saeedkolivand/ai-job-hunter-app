import type { RadarRing } from '@/data/tech-radar';

// Shared ring-shape geometry so a ring is never encoded by color alone: a
// filled circle (Adopt), square (Trial), upward triangle (Assess), diamond
// (Hold). Used both by the small legend icons and by every blip in the main
// radar SVG (RadarSvg composes this inside a translated <g>, so it's drawn
// relative to (0,0) — never give it its own x/y).
export function RadarGlyph({
  ring,
  size = 15,
  className,
}: {
  ring: RadarRing;
  size?: number;
  className?: string;
}) {
  const s = size;
  switch (ring) {
    case 'adopt':
      return <circle className={className} r={s * 0.62} />;
    case 'trial':
      return (
        <rect className={className} x={-s * 0.55} y={-s * 0.55} width={s * 1.1} height={s * 1.1} />
      );
    case 'assess':
      return (
        <polygon
          className={className}
          points={`0,${-s * 0.72} ${s * 0.68},${s * 0.5} ${-s * 0.68},${s * 0.5}`}
        />
      );
    case 'hold':
      return (
        <polygon
          className={className}
          points={`0,${-s * 0.72} ${s * 0.72},0 0,${s * 0.72} ${-s * 0.72},0`}
        />
      );
  }
}
