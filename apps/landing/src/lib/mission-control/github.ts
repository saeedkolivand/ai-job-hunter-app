import { MC_CONFIG } from './config';

// ── The data-source seam ─────────────────────────────────────────────────────
// Every widget reads through `liveOrSnapshot`. In 'snapshot' mode (PR4) the same
// call reads pre-baked nightly JSON from `snapshotBase` (baked by
// metrics-snapshot.yml), falling through to the live API on any miss — config
// change, no rewrite.
type DataSourceMode = 'live' | 'snapshot';
export interface DataSource {
  mode: DataSourceMode;
  snapshotBase?: string;
}

export async function liveOrSnapshot<T>(
  source: DataSource,
  key: string,
  live: () => Promise<T>
): Promise<T> {
  if (source.mode === 'snapshot' && source.snapshotBase) {
    try {
      const res = await fetch(`${source.snapshotBase}/${key}.json`);
      if (res.ok) return (await res.json()) as T;
    } catch {
      // fall through to live on any snapshot miss
    }
  }
  return live();
}

// ── Snapshot freshness (honest-UI: never present nightly data as live) ────────
// metrics-snapshot.yml writes `<snapshotBase>/meta.json` with a `generatedAt` ISO
// stamp. `fetchSnapshotStamp` returns it, or null in live mode / when the snapshot
// is absent (a 404 before the first nightly run). Kept out of the verdict logic —
// it only describes data provenance.
export async function fetchSnapshotStamp(source: DataSource): Promise<string | null> {
  if (source.mode !== 'snapshot' || !source.snapshotBase) return null;
  try {
    const res = await fetch(`${source.snapshotBase}/meta.json`);
    if (!res.ok) return null;
    const meta = (await res.json()) as { generatedAt?: unknown };
    return typeof meta.generatedAt === 'string' ? meta.generatedAt : null;
  } catch {
    return null;
  }
}

const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;
const FRESHNESS_UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ['day', DAY_MS],
  ['hour', HOUR_MS],
  ['minute', MINUTE_MS],
];

// Pure: the muted line "snapshot from <relative time> · data refreshes nightly",
// or null for an unparseable timestamp (render nothing). Locale pinned to 'en' so
// the copy is stable regardless of the runner/browser locale.
export function snapshotFreshnessLine(generatedAt: string, nowMs: number): string | null {
  const ts = Date.parse(generatedAt);
  if (Number.isNaN(ts)) return null;
  const diffMs = ts - nowMs; // negative → in the past
  const [unit, unitMs] = FRESHNESS_UNITS.find(([, ms]) => Math.abs(diffMs) >= ms) ?? [
    'minute',
    MINUTE_MS,
  ];
  const rel = new Intl.RelativeTimeFormat('en', { numeric: 'auto' }).format(
    Math.round(diffMs / unitMs),
    unit
  );
  return `snapshot from ${rel} · data refreshes nightly`;
}

// ── Auth ─────────────────────────────────────────────────────────────────────
// SECURITY: the token is only ever placed in the Authorization header, never in
// a URL, query string, log, or error message. `token === ''` means anonymous.
export function authHeaders(token: string, extra?: Record<string, string>): Record<string, string> {
  const headers: Record<string, string> = {
    Accept: 'application/vnd.github+json',
    'X-GitHub-Api-Version': '2022-11-28',
    ...extra,
  };
  if (token) headers.Authorization = `Bearer ${token}`;
  return headers;
}

// ── Conditional-request cache (localStorage + ETag) ──────────────────────────
interface CacheEntry<T> {
  etag: string | null;
  data: T;
  ts: number;
}

const cacheKey = (path: string): string => MC_CONFIG.cachePrefix + path;

function readCache<T>(path: string): CacheEntry<T> | null {
  try {
    const raw = localStorage.getItem(cacheKey(path));
    if (!raw) return null;
    const entry = JSON.parse(raw) as CacheEntry<T>;
    return entry && typeof entry.ts === 'number' ? entry : null;
  } catch {
    return null;
  }
}

function writeCache<T>(path: string, entry: CacheEntry<T>): void {
  try {
    localStorage.setItem(cacheKey(path), JSON.stringify(entry));
  } catch {
    // storage full / disabled — degrade to no cache
  }
}

// ── Reads ────────────────────────────────────────────────────────────────────
// `path` is always one of our own literal strings (e.g. '/pulls?state=open'), so
// no error message ever leaks caller input, let alone the token.
export async function ghGet<T>(path: string, token: string): Promise<T> {
  const cached = readCache<T>(path);

  // Within the TTL, serve the cache with NO network hit at all — saves a
  // rate-limit unit. Past the TTL we still revalidate cheaply via the ETag below.
  if (cached && Date.now() - cached.ts < MC_CONFIG.cacheTtlMs) {
    return cached.data;
  }

  const headers = authHeaders(token, cached?.etag ? { 'If-None-Match': cached.etag } : undefined);

  let res: Response;
  try {
    res = await fetch(MC_CONFIG.apiBase + path, { headers });
  } catch {
    if (cached) return cached.data;
    throw new Error('Could not reach the GitHub API (network error).');
  }

  // 304 Not Modified — the conditional request saved a body and a rate-limit unit.
  if (res.status === 304 && cached) return cached.data;

  if (res.status === 401) {
    throw new Error('GitHub rejected the token (401). Check its scopes and sign in again.');
  }
  if (res.status === 403 && res.headers.get('X-RateLimit-Remaining') === '0') {
    // Prefer stale cache over an error when rate-limited (same as the generic
    // !res.ok path below) — only throw when there's nothing cached to serve.
    if (cached) return cached.data;
    throw new Error(
      'GitHub API hourly rate limit reached. Sign in with a fine-grained token to raise it to 5,000/h.'
    );
  }
  if (!res.ok) {
    if (cached) return cached.data;
    throw new Error(`GitHub API error ${res.status} for ${path}.`);
  }

  const data = (await res.json()) as T;
  writeCache(path, { etag: res.headers.get('ETag'), data, ts: Date.now() });
  return data;
}

// ── Writes (safe-tier only; every caller confirms first — see write-actions.ts) ─
export interface WriteResult {
  ok: boolean;
  status: number;
}

export async function ghWrite(
  path: string,
  method: 'POST' | 'PATCH',
  body: unknown,
  token: string
): Promise<WriteResult> {
  const res = await fetch(MC_CONFIG.apiBase + path, {
    method,
    headers: authHeaders(token, { 'Content-Type': 'application/json' }),
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  return { ok: res.ok, status: res.status };
}
