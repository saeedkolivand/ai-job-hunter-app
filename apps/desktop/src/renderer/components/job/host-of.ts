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
