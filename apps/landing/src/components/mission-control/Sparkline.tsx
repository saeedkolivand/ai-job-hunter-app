import { sparklineGeometry } from '@/lib/mission-control/sparkline';

// Hand-rolled SVG sparkline — no chart library. Informative SVG: role="img" +
// aria-label. `vectorEffect` keeps the 1.8px stroke crisp under the non-uniform
// viewBox scaling. Series color is passed in (dark-palette desaturated tokens).
export function Sparkline({
  values,
  label,
  width = 260,
  height = 46,
  stroke = 'var(--doc-series-1)',
}: {
  values: readonly number[];
  label: string;
  width?: number;
  height?: number;
  stroke?: string;
}) {
  const geo = sparklineGeometry(values, width, height);
  if (!geo) return <p className="mc-empty">no data yet</p>;

  return (
    <svg
      className="mc-spark"
      viewBox={`0 0 ${width} ${height}`}
      role="img"
      aria-label={label}
      preserveAspectRatio="none"
    >
      <path d={geo.area} fill={stroke} fillOpacity={0.12} stroke="none" />
      <path
        d={geo.line}
        fill="none"
        stroke={stroke}
        strokeWidth={1.8}
        strokeLinecap="round"
        strokeLinejoin="round"
        vectorEffect="non-scaling-stroke"
      />
      {geo.last ? <circle cx={geo.last.x} cy={geo.last.y} r={2.6} fill={stroke} /> : null}
    </svg>
  );
}
