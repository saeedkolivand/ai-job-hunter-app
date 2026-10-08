/**
 * Best-effort host label for a job url when no board id is known — the hostname
 * minus a leading `www.`. Shared by the cross-board cluster surfaces
 * (ClusterSourceChips + the detail pane's "All sources" list) so both render an
 * identical fallback label. Returns the raw string on an unparseable url.
 */
export function hostOf(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, '');
  } catch {
    return url;
  }
}

/**
 * Groups cluster members by board (or host when no board is known), keeping
 * first-seen order, so a cluster whose members all come from one board renders
 * one entry with a count instead of N identical ones.
 */
export function groupByBoard<M extends { board?: string; url: string }>(
  members: M[]
): { id: string; members: M[] }[] {
  const groups = new Map<string, M[]>();
  for (const m of members) {
    const id = m.board?.trim() || hostOf(m.url);
    groups.set(id, [...(groups.get(id) ?? []), m]);
  }
  return [...groups].map(([id, grouped]) => ({ id, members: grouped }));
}
