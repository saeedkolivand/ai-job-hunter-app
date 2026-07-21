import { MC_CONFIG } from './config';

// ── The data-source seam ─────────────────────────────────────────────────────
// Every widget reads through `liveOrSnapshot`. Today it always goes live; PR4's
// nightly snapshot plane flips `MC_CONFIG.dataSource.mode` to 'snapshot' and the
// same call reads pre-baked JSON from `snapshotBase` — config change, no rewrite.
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
