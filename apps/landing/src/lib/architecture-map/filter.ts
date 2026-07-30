import { FIXES, KNOWN_BUGS, type MapEdge, type MapNode } from '@/data/architecture-map';

// Adjacency for selection highlighting — who's one hop from whom.
export function buildAdjacency(
  nodeList: readonly MapNode[],
  edgeList: readonly MapEdge[]
): Map<string, Set<string>> {
  const adj = new Map<string, Set<string>>();
  for (const n of nodeList) adj.set(n.id, new Set());
  for (const e of edgeList) {
    adj.get(e.from)?.add(e.to);
    adj.get(e.to)?.add(e.from);
  }
  return adj;
}

export function hasBadge(id: string): boolean {
  return Boolean(FIXES[id] ?? KNOWN_BUGS[id]);
}

// `filter` is the interaction engine's mutable view-state string ('overview' |
// 'all' | 'bugs' | a tag id) — passed in rather than closed over so these stay
// pure, testable functions.
export function passNodeFilter(n: MapNode, filter: string): boolean {
  if (filter === 'all' || filter === 'overview') return true;
  if (filter === 'bugs') return hasBadge(n.id);
  return n.tag.includes(filter);
}

export function passEdgeFilter(e: MapEdge, filter: string): boolean {
  if (filter === 'all') return true;
  if (filter === 'overview') return e.kind !== 'normal' || e.tag.includes('overview');
  if (filter === 'bugs') return false;
  return e.tag.includes(filter);
}
