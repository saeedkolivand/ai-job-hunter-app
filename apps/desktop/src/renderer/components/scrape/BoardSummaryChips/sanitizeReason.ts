/** Max length of a sanitized reason — a hint, not a full error dump. */
const MAX_REASON_LEN = 200;

/** Leading/trailing wrapping punctuation to strip before classifying a token. */
const TRIM_PUNCT = /^["'`([\]{}<>|,;:]+|["'`()[\]{}<>|,;:]+$/g;

/**
 * Client-side mirror of the Rust `sanitize_reason` intent: an error string that
 * crossed IPC (or was persisted to disk) may carry absolute paths, full URLs,
 * host:port, emails, or credential fragments. We do NOT trust persisted strings
 * (PR B carry-over 4), so each whitespace token that looks like one of those is
 * collapsed to a neutral placeholder before display; the human message around it
 * (e.g. `"429 Too Many Requests"`) is kept intact. Over-redaction is always safe.
 */
export function sanitizeReason(raw: string): string {
  if (typeof raw !== 'string') return '';
  // Pre-cap the INPUT before tokenizing (independent of the MAX_REASON_LEN
  // truncation of the OUTPUT below) so a pathological multi-megabyte string
  // can't force an unbounded split/map over the full length.
  const capped = raw.length > 1000 ? raw.slice(0, 1000) : raw;
  const out = capped.split(/\s+/).filter(Boolean).map(redactToken).join(' ');
  return out.length > MAX_REASON_LEN ? `${out.slice(0, MAX_REASON_LEN)}…` : out;
}

/** Classify a single token and swap it for a placeholder when it leaks context. */
function redactToken(token: string): string {
  const trimmed = token.replace(TRIM_PUNCT, '');
  if (!trimmed) return token;
  const lower = trimmed.toLowerCase();

  const isUrl = trimmed.includes('://');
  const isCredential = [
    'key=',
    'app_id=',
    'secret=',
    'token=',
    'password=',
    'pwd=',
    'auth=',
    'key":',
    'secret":',
    'token":',
    'password":',
    'auth":',
  ].some((marker) => lower.includes(marker));
  const isWindowsPath = /^[a-z]:[\\/]/i.test(trimmed);
  const isUnixPath = trimmed.startsWith('/') && trimmed.slice(1).includes('/');
  const isHomeish =
    lower.includes('users\\') || lower.includes('users/') || lower.includes('home/');
  // UNC network path — `\\server\share\...` — leaks the user's network layout
  // same as a local absolute path.
  const isUncPath = trimmed.startsWith('\\\\');

  const segs = trimmed.split('.').filter(Boolean);
  const dottedIpv4 = segs.length === 4 && segs.every((seg) => /^\d+$/.test(seg));
  const colonIdx = trimmed.lastIndexOf(':');
  const preColon = colonIdx > 0 ? trimmed.slice(0, colonIdx) : '';
  const preColonSegs = preColon.split('.').filter(Boolean);
  // A bare "host:port" needs a hostname-LIKE part before the colon — either a
  // dotted-IPv4 (4 numeric octets) or something with at least one letter.
  // Without this, a ratio like "3.5:1" (2 numeric segments, no letters) was
  // misclassified as host:port and redacted.
  const preColonIsHostLike =
    preColon.includes('.') &&
    (preColonSegs.length === 4 && preColonSegs.every((s) => /^\d+$/.test(s))
      ? true
      : /[a-z]/i.test(preColon));
  const hostPort = colonIdx > 0 && preColonIsHostLike && /^\d+$/.test(trimmed.slice(colonIdx + 1));
  const isHostPort = trimmed.includes('.') && (dottedIpv4 || hostPort);

  const atIdx = trimmed.indexOf('@');
  const isEmail = atIdx > 0 && trimmed.slice(atIdx + 1).includes('.');

  let placeholder: string | null = null;
  if (isUrl) placeholder = '<url-redacted>';
  else if (isCredential) placeholder = '<credential-redacted>';
  else if (isWindowsPath || isUnixPath || isHomeish || isUncPath) placeholder = '<path-redacted>';
  else if (isHostPort) placeholder = '<host-redacted>';
  else if (isEmail) placeholder = '<email-redacted>';

  return placeholder ? token.replace(trimmed, placeholder) : token;
}
