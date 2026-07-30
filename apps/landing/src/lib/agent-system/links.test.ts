import { describe, expect, it } from 'vitest';

import { linkPathD, type LinkRect } from './links';

/** Parses `M sx sy C c1x c1y c2x c2y ex ey` into its numeric parts. */
function parseD(d: string) {
  const match =
    /^M(-?[\d.]+) (-?[\d.]+) C(-?[\d.]+) (-?[\d.]+) (-?[\d.]+) (-?[\d.]+) (-?[\d.]+) (-?[\d.]+)$/.exec(
      d
    );
  if (!match) throw new Error(`unparseable path d: ${d}`);
  const [, sx, sy, c1x, c1y, c2x, c2y, ex, ey] = match;
  return {
    sx: Number(sx),
    sy: Number(sy),
    c1x: Number(c1x),
    c1y: Number(c1y),
    c2x: Number(c2x),
    c2y: Number(c2y),
    ex: Number(ex),
    ey: Number(ey),
  };
}

const grid = { left: 100, top: 50 };

function rect(overrides: Partial<LinkRect> = {}): LinkRect {
  return { left: 0, right: 0, top: 0, height: 0, ...overrides };
}

describe('linkPathD', () => {
  it('starts at the author rect right-edge midpoint, relative to the grid', () => {
    const author = rect({ left: 120, right: 220, top: 60, height: 40 });
    const critic = rect({ left: 400, right: 500, top: 60, height: 40 });
    const { sx, sy } = parseD(linkPathD(grid, author, critic));
    expect(sx).toBe(author.right - grid.left);
    expect(sy).toBe(author.top + author.height / 2 - grid.top);
  });

  it('ends at the critic rect left-edge midpoint, relative to the grid', () => {
    const author = rect({ left: 120, right: 220, top: 60, height: 40 });
    const critic = rect({ left: 400, right: 500, top: 90, height: 60 });
    const { ex, ey } = parseD(linkPathD(grid, author, critic));
    expect(ex).toBe(critic.left - grid.left);
    expect(ey).toBe(critic.top + critic.height / 2 - grid.top);
  });

  it('is a cubic whose control points exit/enter horizontally (c1y === sy, c2y === ey)', () => {
    const author = rect({ left: 120, right: 220, top: 60, height: 40 });
    const critic = rect({ left: 400, right: 500, top: 300, height: 60 });
    const { sy, ey, c1y, c2y } = parseD(linkPathD(grid, author, critic));
    expect(c1y).toBe(sy);
    expect(c2y).toBe(ey);
  });

  it('places both control points at the horizontal midpoint between start and end', () => {
    const author = rect({ left: 120, right: 220, top: 60, height: 40 });
    const critic = rect({ left: 400, right: 500, top: 300, height: 60 });
    const { sx, ex, c1x, c2x } = parseD(linkPathD(grid, author, critic));
    const mx = (sx + ex) / 2;
    expect(c1x).toBe(mx);
    expect(c2x).toBe(mx);
  });

  it('collapses to a single point when source and target rects are identical', () => {
    const same = rect({ left: 200, right: 200, top: 60, height: 40 });
    const { sx, sy, c1x, c1y, c2x, c2y, ex, ey } = parseD(linkPathD(grid, same, same));
    expect([sx, c1x, c2x, ex].every((x) => x === sx)).toBe(true);
    expect([sy, c1y, c2y, ey].every((y) => y === sy)).toBe(true);
  });

  it('handles zero-size rects (all coordinates collapse to the grid origin)', () => {
    const zero = rect();
    const d = linkPathD({ left: 0, top: 0 }, zero, zero);
    expect(d).toBe('M0 0 C0 0 0 0 0 0');
  });

  it('is relative to the grid origin, not absolute page coordinates', () => {
    const author = rect({ left: 300, right: 400, top: 150, height: 50 });
    const critic = rect({ left: 600, right: 700, top: 150, height: 50 });
    const atOrigin = linkPathD({ left: 0, top: 0 }, author, critic);
    const shifted = linkPathD(
      { left: 100, top: 20 },
      rect({ ...author, left: author.left + 100, right: author.right + 100, top: author.top + 20 }),
      rect({ ...critic, left: critic.left + 100, right: critic.right + 100, top: critic.top + 20 })
    );
    expect(shifted).toBe(atOrigin);
  });
});
