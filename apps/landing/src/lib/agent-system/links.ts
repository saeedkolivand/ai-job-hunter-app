// Pure fleet-map link geometry — the bezier path between an author node's
// right edge and a critic node's left edge, both rects already measured
// (getBoundingClientRect) and passed in relative to nothing in particular;
// this function does the relative-to-grid math. DOM querying + state lives
// in components/agent-system/hooks.ts (useMapLinks).

export interface LinkRect {
  left: number;
  right: number;
  top: number;
  height: number;
}

export function linkPathD(
  gridRect: { left: number; top: number },
  authorRect: LinkRect,
  criticRect: LinkRect
): string {
  const ax = authorRect.right - gridRect.left;
  const ay = authorRect.top + authorRect.height / 2 - gridRect.top;
  const cx = criticRect.left - gridRect.left;
  const cy = criticRect.top + criticRect.height / 2 - gridRect.top;
  const mx = (ax + cx) / 2;
  return `M${ax} ${ay} C${mx} ${ay} ${mx} ${cy} ${cx} ${cy}`;
}
