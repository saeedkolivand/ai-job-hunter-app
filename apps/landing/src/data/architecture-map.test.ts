import { describe, expect, it } from 'vitest';

import {
  CHIPS,
  clusters,
  COLORS,
  edges,
  FINDINGS,
  FIXES,
  KNOWN_BUGS,
  nodes,
} from './architecture-map';

// Typed-data invariants — "does every id/tag this graph cites actually resolve
// to something real", the same class of rot that produced 2 dead edges in the
// original hand-authored dashboard. scripts/check-landing-drift.mjs covers
// cited file paths, IPC contract names, and dead prose terms; this file covers
// the graph's own internal cross-references, which the drift guard never reads.
describe('architecture-map data integrity', () => {
  const nodeIds = new Set(nodes.map((n) => n.id));
  const clusterIds = new Set(clusters.map((c) => c.id));
  const chipIds = new Set(CHIPS.map(([id]) => id));
  // 'dashboard' and 'support' are descriptive tags with no dedicated filter
  // chip — verbatim from the pre-port dashboard (apps/landing/public/
  // architecture-map.html, since removed) for the Dashboard route and the
  // Support/diagnostics screens. Not chip-driven (see passNodeFilter in
  // ArchitectureMap.tsx), so absent from CHIPS by design — not a typo.
  const knownTagIds = new Set([...chipIds, 'dashboard', 'support']);

  it('keeps a sane floor on node/cluster/edge counts', () => {
    // Floor, not an exact pin — this graph is living data (currently 142/11/128);
    // the point is catching "got emptied by accident", not fighting every
    // legitimate addition/removal with a hardcoded count.
    expect(nodes.length).toBeGreaterThan(100);
    expect(clusters.length).toBeGreaterThan(8);
    expect(edges.length).toBeGreaterThan(100);
  });

  it('gives every node a unique id', () => {
    expect(nodeIds.size).toBe(nodes.length);
  });

  it('gives every cluster a unique id', () => {
    expect(clusterIds.size).toBe(clusters.length);
  });

  it('resolves every node cluster to a real cluster id', () => {
    for (const n of nodes) expect(clusterIds.has(n.cluster)).toBe(true);
  });

  it('resolves every edge endpoint to a real node id', () => {
    for (const e of edges) {
      expect(nodeIds.has(e.from)).toBe(true);
      expect(nodeIds.has(e.to)).toBe(true);
    }
  });

  it('resolves every FIXES key to a real node id', () => {
    for (const id of Object.keys(FIXES)) expect(nodeIds.has(id)).toBe(true);
  });

  it('resolves every KNOWN_BUGS key to a real node id', () => {
    for (const id of Object.keys(KNOWN_BUGS)) expect(nodeIds.has(id)).toBe(true);
  });

  it('keeps FINDINGS as non-empty prose, not structured node references', () => {
    // FINDINGS is verbatim sidebar prose (it may *name* a node id inside a
    // sentence, e.g. "Flagged red on cmd-match") — not an id-keyed map like
    // FIXES/KNOWN_BUGS, so there is nothing to resolve against nodeIds here;
    // just guard the shape stays a non-empty array of strings.
    expect(FINDINGS.length).toBeGreaterThan(0);
    for (const finding of FINDINGS) expect(typeof finding).toBe('string');
  });

  it('balances every <b>/</b> pair in FINDINGS prose', () => {
    // renderRich() only matches complete <b>…</b> pairs and turns them into
    // React elements; a dangling <b> (typo'd/unclosed) falls through as a
    // literal, visible "<b>" in the sidebar instead of emphasis.
    for (const finding of FINDINGS) {
      const opens = finding.split('<b>').length - 1;
      const closes = finding.split('</b>').length - 1;
      expect(opens).toBe(closes);
    }
  });

  it('tags every node and edge with a known CHIPS filter id (or documented extra)', () => {
    for (const n of nodes) {
      for (const t of n.tag) expect(knownTagIds.has(t)).toBe(true);
    }
    for (const e of edges) {
      for (const t of e.tag) expect(knownTagIds.has(t)).toBe(true);
    }
  });

  it('tags every node and edge with the universal "all" filter', () => {
    // passNodeFilter/passEdgeFilter treat 'all' as the show-everything filter —
    // an entry missing it would be unreachable from the "Show all wires" chip,
    // the same class of rot that produced 2 dead edges in the original.
    for (const n of nodes) expect(n.tag).toContain('all');
    for (const e of edges) expect(e.tag).toContain('all');
  });

  it('gives every non-bugs CHIPS filter at least one matching node and edge', () => {
    // 'bugs' is derived from FIXES/KNOWN_BUGS membership (see hasBadge() in
    // ArchitectureMap.tsx), not from tag membership, so it never appears as a
    // literal tag value and is excluded from this completeness check.
    const routedChipIds = CHIPS.map(([id]) => id).filter((id) => id !== 'bugs');
    for (const id of routedChipIds) {
      expect(nodes.some((n) => n.tag.includes(id))).toBe(true);
      expect(edges.some((e) => e.tag.includes(id))).toBe(true);
    }
  });

  it('covers every cluster/node color with a COLORS hex value', () => {
    for (const c of clusters) expect(COLORS[c.color]).toBeTruthy();
    for (const n of nodes) expect(COLORS[n.color]).toBeTruthy();
  });
});
