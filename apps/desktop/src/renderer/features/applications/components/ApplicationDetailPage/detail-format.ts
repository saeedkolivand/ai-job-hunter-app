import { EVENT_SOURCE_EMAIL, EVENT_SOURCE_EMAIL_REJECT, type StatusEvent } from '@ajh/shared';

/** http(s)-only guard — mirrors ApplicationRow's open-link security gate. */
export const isHttpUrl = (url: string) => /^https?:\/\//i.test(url);

/** Format an epoch-ms timestamp for a `<input type="date">` value (YYYY-MM-DD, local). */
export function toDateInputValue(ms?: number): string {
  if (!ms) return '';
  const d = new Date(ms);
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, '0');
  return `${y}-${m}-${day}`;
}

/** Parse a `<input type="date">` value to local start-of-day epoch ms, or null when empty. */
export function fromDateInputValue(value: string): number | null {
  if (!value) return null;
  const parts = value.split('-').map(Number);
  const [y, m, d] = parts;
  if (!y || !m || !d) return null;
  return new Date(y, m - 1, d).getTime();
}

export function formatEventDate(at: number): string {
  return new Date(at).toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

/** Map an application status to a Timeline dot colour (graceful substring match). */
function statusColor(status: string): 'red' | 'green' | 'blue' | 'brand' {
  const s = status.toLowerCase();
  if (/reject|declin|withdraw/.test(s)) return 'red';
  if (/offer|accept|hire/.test(s)) return 'green';
  if (/interview|screen/.test(s)) return 'blue';
  return 'brand';
}

/** An unconfirmed email-derived write — the only kind the timeline renders as
 *  provisional (Accept/Reject affordances, never presented as settled history). */
export function isProvisionalEvent(e: StatusEvent): boolean {
  return e.source === EVENT_SOURCE_EMAIL && !e.confirmed;
}

/** The reversal row `rejectStatusEvent` appends when its compare-and-set wins —
 *  a correction in the trail, distinct from both a normal user transition and
 *  the provisional row it resolves. */
export function isCorrectionEvent(e: StatusEvent): boolean {
  return e.source === EVENT_SOURCE_EMAIL_REJECT;
}

/** Provisional/correction rows read as unsettled — same muted grey as the
 *  Timeline's own pending-ghost node — rather than claiming a status colour. */
export function timelineEventColor(e: StatusEvent): 'red' | 'green' | 'blue' | 'brand' | 'gray' {
  if (isProvisionalEvent(e) || isCorrectionEvent(e)) return 'gray';
  return statusColor(e.toStatus);
}
