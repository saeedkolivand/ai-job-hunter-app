import { describe, expect, it } from 'vitest';

import { FIXES, KNOWN_BUGS, type MapEdge, type MapNode } from '@/data/architecture-map';

import { buildAdjacency, hasBadge, passEdgeFilter, passNodeFilter } from './filter';

function node(overrides: Partial<MapNode> = {}): MapNode {
  return {
    id: 'n',
    cluster: 'client',
    label: 'Node',
    sub: '',
    x: 0,
    y: 0,
    w: 100,
    h: 40,
    color: 'client',
    role: '',
    plain: '',
    path: '',
    notes: [],
    tag: [],
    ...overrides,
  };
}

function edge(overrides: Partial<MapEdge> = {}): MapEdge {
  return { from: 'a', to: 'b', kind: 'normal', tag: [], ...overrides };
}

// Real ids from architecture-map.ts's FIXES/KNOWN_BUGS data, so hasBadge (and
// anything built on it) is exercised against the actual dataset rather than a
// hand-rolled stand-in.
const FIX_ONLY_ID = Object.keys(FIXES)[0];
const BUG_ONLY_ID = Object.keys(KNOWN_BUGS)[0];
const NO_BADGE_ID = 'no-such-node-id';

if (!FIX_ONLY_ID || !BUG_ONLY_ID) {
  throw new Error('expected FIXES and KNOWN_BUGS to be non-empty in the fixture dataset');
}

describe('buildAdjacency', () => {
  it('maps every node to its neighbors in both directions', () => {
    const nodes = [node({ id: 'a' }), node({ id: 'b' }), node({ id: 'c' })];
    const edges = [edge({ from: 'a', to: 'b' })];
    const adj = buildAdjacency(nodes, edges);
    expect(adj.get('a')).toEqual(new Set(['b']));
    expect(adj.get('b')).toEqual(new Set(['a']));
    expect(adj.get('c')).toEqual(new Set()); // present, but no neighbors
  });

  it('every declared node gets an entry even with an empty edge list', () => {
    const nodes = [node({ id: 'a' }), node({ id: 'b' })];
    const adj = buildAdjacency(nodes, []);
    expect([...adj.keys()]).toEqual(['a', 'b']);
    expect(adj.get('a')).toEqual(new Set());
  });

  it('returns an empty map for an empty node list, ignoring dangling edges silently', () => {
    const adj = buildAdjacency([], [edge({ from: 'a', to: 'b' })]);
    expect(adj.size).toBe(0);
  });

  it('dedupes a self-loop and repeated edges via the underlying Set', () => {
    const nodes = [node({ id: 'a' }), node({ id: 'b' })];
    const edges = [
      edge({ from: 'a', to: 'a' }),
      edge({ from: 'a', to: 'b' }),
      edge({ from: 'a', to: 'b' }),
    ];
    const adj = buildAdjacency(nodes, edges);
    expect(adj.get('a')).toEqual(new Set(['a', 'b']));
  });
});

describe('hasBadge', () => {
  it('is true for an id with a planned fix, an id with a known bug, and false otherwise', () => {
    expect(hasBadge(FIX_ONLY_ID)).toBe(true);
    expect(hasBadge(BUG_ONLY_ID)).toBe(true);
    expect(hasBadge(NO_BADGE_ID)).toBe(false);
  });
});

describe('passNodeFilter', () => {
  it.each(['all', 'overview'] as const)('"%s" passes every node regardless of tag', (filter) => {
    expect(passNodeFilter(node({ tag: [] }), filter)).toBe(true);
    expect(passNodeFilter(node({ tag: ['scraper'] }), filter)).toBe(true);
  });

  it('"bugs" passes only nodes with a fix or bug badge', () => {
    expect(passNodeFilter(node({ id: FIX_ONLY_ID }), 'bugs')).toBe(true);
    expect(passNodeFilter(node({ id: BUG_ONLY_ID }), 'bugs')).toBe(true);
    expect(passNodeFilter(node({ id: NO_BADGE_ID }), 'bugs')).toBe(false);
  });

  it('a tag id passes only nodes whose tag array includes it', () => {
    expect(passNodeFilter(node({ tag: ['scraper', 'core'] }), 'scraper')).toBe(true);
    expect(passNodeFilter(node({ tag: ['core'] }), 'scraper')).toBe(false);
    expect(passNodeFilter(node({ tag: [] }), 'scraper')).toBe(false);
  });
});

describe('passEdgeFilter', () => {
  it('"all" passes every edge', () => {
    expect(passEdgeFilter(edge({ kind: 'normal', tag: [] }), 'all')).toBe(true);
  });

  it('"bugs" fails every edge unconditionally', () => {
    expect(passEdgeFilter(edge({ kind: 'critical', tag: ['overview'] }), 'bugs')).toBe(false);
  });

  describe('"overview" — either endpoint of the OR can pass it', () => {
    it('passes when kind !== "normal", even with no overview tag', () => {
      expect(passEdgeFilter(edge({ kind: 'critical', tag: [] }), 'overview')).toBe(true);
    });

    it('passes a "normal" kind edge when the tag includes "overview"', () => {
      expect(passEdgeFilter(edge({ kind: 'normal', tag: ['overview'] }), 'overview')).toBe(true);
    });

    it('passes when both sides of the OR are true', () => {
      expect(passEdgeFilter(edge({ kind: 'critical', tag: ['overview'] }), 'overview')).toBe(true);
    });

    it('fails a "normal" kind edge whose tag omits "overview"', () => {
      expect(passEdgeFilter(edge({ kind: 'normal', tag: ['support'] }), 'overview')).toBe(false);
    });
  });

  it('a tag id passes only edges whose tag array includes it', () => {
    expect(passEdgeFilter(edge({ tag: ['support', 'all'] }), 'support')).toBe(true);
    expect(passEdgeFilter(edge({ tag: ['all'] }), 'support')).toBe(false);
  });
});
