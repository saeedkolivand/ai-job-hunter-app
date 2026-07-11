/**
 * App-wide canonical dedup key for a job posting — the TS mirror of the Rust
 * `canonical_job_key` (`apps/desktop/src-tauri/src/scraping/boards/common.rs`),
 * which itself builds on `normalize_job_url`
 * (`apps/desktop/src-tauri/src/applications/mod.rs`).
 *
 * **LOCKSTEP PAIR** — this file and `scraping/boards/common.rs::canonical_job_key`
 * MUST stay byte-identical for the same inputs (like `sanitizeReason` ↔
 * `redact_token`). The whole point of trust-program PR E is that the engine's
 * cross-source dedup, autopilot's `merge_found_jobs`, and the renderer's
 * `mergePostings` all key on ONE notion of "same job", so a posting surfaced by
 * two boards collapses to one row and fires one notification. If the two sides
 * drift, the deduped completion count no longer reconciles with the displayed
 * rows. The truth-table test copies the Rust test inputs verbatim so any drift
 * fails a test.
 *
 * Deliberately a raw-string mirror, NOT the WHATWG `URL` API: `normalize_job_url`
 * lowercases the WHOLE url (path + query included), does no percent-decoding, and
 * strips trailing slashes itself. `URL` lowercases only scheme+host, re-encodes
 * components, and resolves `.`/`..` — any of which would diverge from the Rust
 * key for real inputs, defeating the cross-boundary dedup this exists to enable.
 */

/**
 * Extract an explicit URL scheme (`scheme:` per RFC 3986 §3.1), lowercased, if
 * one is present before the first `/`, `?`, or `#`. Mirrors Rust
 * `explicit_scheme`. Returns `null` for scheme-less input (a `:` inside a path or
 * query is not a scheme).
 */
function explicitScheme(input: string): string | null {
  // Only the head before the first path/query/fragment delimiter can carry a scheme.
  const head = input.split(/[/?#]/, 1)[0] ?? input;
  const colon = head.indexOf(':');
  if (colon === -1) return null;
  const candidate = head.slice(0, colon);
  if (candidate === '') return null;
  // First char ALPHA, rest ALPHA / DIGIT / "+" / "-" / ".".
  if (!/^[A-Za-z][A-Za-z0-9+.-]*$/.test(candidate)) return null;
  return candidate.toLowerCase();
}

/**
 * Per-host allowlist of *identifying* query params that survive normalization
 * (every other query param is dropped). Mirrors Rust `identifying_query_params`:
 * currently only Indeed's `jk` (other boards put the id in the path).
 */
function identifyingQueryParams(host: string): readonly string[] {
  if (host === 'indeed.com' || host.endsWith('.indeed.com')) return ['jk'];
  return [];
}

/**
 * Rebuild the query keeping only `identifyingQueryParams(host)`, emitted in the
 * allowlist's fixed order (so input param ordering can't change the key). A param
 * with an empty value is skipped. Mirrors Rust `retain_identifying_params`.
 */
function retainIdentifyingParams(host: string, query: string): string {
  const allow = identifyingQueryParams(host);
  if (allow.length === 0 || query === '') return '';
  const parts: string[] = [];
  for (const key of allow) {
    for (const pair of query.split('&')) {
      const eq = pair.indexOf('=');
      const k = eq === -1 ? pair : pair.slice(0, eq);
      const v = eq === -1 ? '' : pair.slice(eq + 1);
      if (k === key && v !== '') {
        parts.push(`${key}=${v}`);
        break; // first match per key, like Rust's find_map
      }
    }
  }
  return parts.join('&');
}

/**
 * Normalize a job URL into a stable identity string. Mirrors Rust
 * `normalize_job_url`: rejects any non-`http(s)` explicit scheme to `""`;
 * lowercases the whole URL; strips a leading `www.` on the host; drops the
 * `#fragment`; drops the query except per-host identifying params; strips a
 * trailing `/` on the path. Empty/blank input → `""`.
 */
function normalizeJobUrl(url: string): string {
  const trimmed = url.trim();
  if (trimmed === '') return '';

  // Reject dangerous explicit schemes at the single chokepoint: only http(s) round-trips.
  const scheme = explicitScheme(trimmed);
  if (scheme !== null && scheme !== 'http' && scheme !== 'https') return '';

  const lower = trimmed.toLowerCase();
  // split_once("://")
  const sep = lower.indexOf('://');
  const schemePart = sep === -1 ? null : lower.slice(0, sep);
  const rest = sep === -1 ? lower : lower.slice(sep + 3);

  // Drop the fragment, then split the query off the path.
  const hash = rest.indexOf('#');
  const noFrag = hash === -1 ? rest : rest.slice(0, hash);
  const qmark = noFrag.indexOf('?');
  const pathPart = qmark === -1 ? noFrag : noFrag.slice(0, qmark);
  const query = qmark === -1 ? '' : noFrag.slice(qmark + 1);

  // split_once('/') — host, then everything after the first slash (or none).
  const slash = pathPart.indexOf('/');
  const rawHost = slash === -1 ? pathPart : pathPart.slice(0, slash);
  const path = slash === -1 ? null : pathPart.slice(slash + 1);

  const host = rawHost.startsWith('www.') ? rawHost.slice(4) : rawHost;
  const retainedQuery = retainIdentifyingParams(host, query);

  let out = '';
  if (schemePart !== null) out += `${schemePart}://`;
  out += host;
  if (path !== null) {
    const trimmedPath = path.replace(/\/+$/, ''); // Rust trim_end_matches('/')
    if (trimmedPath !== '') out += `/${trimmedPath}`;
  }
  if (retainedQuery !== '') out += `?${retainedQuery}`;
  return out;
}

/**
 * U+0001 (SOH) fallback-key separator, matching Rust's `\u{1}`. Cannot occur in
 * real title/company text, so a title that merely contains the company name
 * can't forge a colliding key, and near-miss titles stay distinct.
 */
const KEY_SEP = String.fromCharCode(1); // U+0001 (SOH); see doc above

/**
 * The canonical dedup key. Mirrors Rust `canonical_job_key`:
 * 1. `n = normalizeJobUrl(url)`; if non-empty, the key IS `n` (URL identity).
 * 2. Otherwise fall back to `"{title}<U+0001>{company}"`, each side
 *    `.trim().toLowerCase()`.
 */
export function canonicalJobKey(url: string, title: string, company: string): string {
  const normalized = normalizeJobUrl(url);
  if (normalized !== '') return normalized;
  return `${title.trim().toLowerCase()}${KEY_SEP}${company.trim().toLowerCase()}`;
}
