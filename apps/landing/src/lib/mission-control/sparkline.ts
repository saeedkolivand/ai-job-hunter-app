// Pure geometry for the hand-rolled SVG sparklines — no chart library. Maps a
// series of numbers onto an SVG polyline path within a width×height box, with a
// small vertical inset so the stroke never clips at the extremes.

export interface SparklineGeometry {
  line: string; // the polyline `d`
  area: string; // a closed path down to the baseline (for a soft fill)
  points: { x: number; y: number }[];
  last: { x: number; y: number } | null;
}

export function sparklineGeometry(
  values: readonly number[],
  width: number,
  height: number,
  inset = 3
): SparklineGeometry | null {
  if (values.length === 0) return null;

  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min || 1; // flat series → a centered horizontal line
  const usable = Math.max(1, height - inset * 2);
  const stepX = values.length > 1 ? width / (values.length - 1) : 0;

  const points = values.map((value, index) => {
    const x = values.length > 1 ? index * stepX : width / 2;
    // Invert Y: higher value = higher on screen (smaller y).
    const y = inset + (1 - (value - min) / span) * usable;
    return { x, y };
  });

  const first = points[0];
  if (!first) return null;
  const line = points
    .map((p, i) => `${i === 0 ? 'M' : 'L'}${p.x.toFixed(1)} ${p.y.toFixed(1)}`)
    .join(' ');
  const last = points[points.length - 1] ?? first;
  const area = `${line} L${last.x.toFixed(1)} ${height} L${first.x.toFixed(1)} ${height} Z`;

  return { line, area, points, last };
}
