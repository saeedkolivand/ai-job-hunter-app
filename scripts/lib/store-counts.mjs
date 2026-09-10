// Store-side counters for the public "installs" number — everything besides
// the GitHub installer downloads that scripts/lib/github-releases.mjs already
// computes.
//
// UNIT CAVEAT: a Microsoft Store "acquisition", a Snap "installed device", a
// Chrome Web Store "weekly active user" and a Firefox AMO "average daily user"
// are four different metrics, not four downloads. None of them is directly
// comparable to a GitHub asset download either. Summing them is still honest
// under one reading — every term is an install by SOME definition, and the sum
// undercounts if anything (an acquisition that never launches the app still
// isn't double-counted anywhere else) — which is exactly why the public label
// is "installs", never "downloads". See build-repo-charts.mjs for where the
// sum happens.
//
// SOFT-FAIL CONTRACT: a store outage, a missing credential, or an upstream
// markup change must never break the nightly badge build. Every fetcher below
// catches everything and returns `null` on failure; `totalInstalls` treats
// `null` as "no data", not zero; `collectStoreCounts` never rejects.

import { execFile } from 'node:child_process';
import { promisify } from 'node:util';

const execFileAsync = promisify(execFile);

// Must match MSSTORE_PRODUCT_ID in .github/workflows/release.yml — same app,
// two separate pipelines (publish vs. read-only metrics).
const MS_STORE_APP_ID = '9NC5KDJV0BTM';
// The listing went live 2026-09-09; started a week early on purpose so a clock
// skew or a late first read can never fall outside the window.
const MS_STORE_START_DATE = '2026-09-01';

const CHROME_USER_AGENT =
  'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36';

// ── Pure parsers ─────────────────────────────────────────────────────────────
// Each returns an integer >= 0, or `null` when the input is unrecognized. No
// network, no process spawning — safe to unit-test directly.

/** AMO's `average_daily_users` field, if it is a usable number. */
export function parseFirefoxUsers(json) {
  const n = json?.average_daily_users;
  return typeof n === 'number' && Number.isFinite(n) && n >= 0 ? Math.round(n) : null;
}

/**
 * Scrapes a Chrome Web Store listing page for its "N users" figure — there is
 * no public API, so the rendered HTML is the only source. Handles the K/M
 * suffix larger listings render (`10K+ users`, `2.5M users`) alongside the
 * plain form (`21 users`, `1,234 users`).
 */
export function parseChromeUsers(html) {
  const m = /([\d,.]+)\s*([KM])?\+?\s*users/i.exec(html ?? '');
  if (!m) return null;
  const n = Number.parseFloat(m[1].replace(/,/g, ''));
  if (!Number.isFinite(n) || n < 0) return null;
  const multiplier =
    m[2]?.toUpperCase() === 'K' ? 1_000 : m[2]?.toUpperCase() === 'M' ? 1_000_000 : 1;
  const result = Math.round(n * multiplier);
  // Sanity ceiling: guards against matching some other number on the page.
  return result > 5_000_000 ? null : result;
}

/**
 * Snap Store metrics API response → installed devices on the latest day.
 * Shape is `{ metrics: [M] }` or a bare `M`, where `M.series[*].values` aligns
 * with `M.buckets` by index; sums every series' LAST bucket only (today).
 */
export function snapInstalledBase(json) {
  const metric = Array.isArray(json?.metrics) ? json.metrics[0] : json;
  if (!metric || !Array.isArray(metric.series) || !Array.isArray(metric.buckets)) return null;
  if (metric.buckets.length === 0) return null;
  if (typeof metric.status === 'string' && metric.status !== 'OK') return null;

  const hasReading = (i) =>
    metric.series.some((s) => {
      const v = s?.values?.[i];
      return typeof v === 'number' && Number.isFinite(v);
    });
  // An all-null latest day must read as "unavailable", not 0 — walk back to
  // the most recent bucket that actually has a reading.
  let last = metric.buckets.length - 1;
  while (last >= 0 && !hasReading(last)) last -= 1;
  if (last < 0) return null;

  const total = metric.series.reduce((sum, s) => sum + (Number(s?.values?.[last]) || 0), 0);
  return total < 0 ? null : total;
}

/** Sum of `acquisitionQuantity` across every paginated Microsoft Store response. */
export function sumAcquisitions(pages) {
  let sum = 0;
  let sawValueArray = false;
  for (const page of pages ?? []) {
    if (!Array.isArray(page?.Value)) continue;
    sawValueArray = true;
    for (const row of page.Value) {
      const q = row?.acquisitionQuantity;
      if (typeof q === 'number' && Number.isFinite(q) && q >= 0) sum += q;
    }
  }
  return sawValueArray ? sum : null;
}

/** Sum of the finite non-null values in `parts` — an all-null input is 0, not null. */
export function totalInstalls(parts) {
  return Object.values(parts).reduce(
    (sum, v) => (typeof v === 'number' && Number.isFinite(v) ? sum + v : sum),
    0
  );
}

// ── Fetchers ─────────────────────────────────────────────────────────────────
// Each is independently soft-failing: a timeout, a non-2xx, a markup change or
// a missing credential all degrade to `null`, never a thrown error.

/** Firefox AMO — no auth, public API. */
export async function fetchFirefoxUsers(slug = 'ai-job-hunter') {
  try {
    const res = await fetch(`https://addons.mozilla.org/api/v5/addons/addon/${slug}/`, {
      signal: AbortSignal.timeout(20_000),
    });
    if (!res.ok) return null;
    return parseFirefoxUsers(await res.json());
  } catch {
    return null;
  }
}

/** Chrome Web Store — no API; scrapes the public listing page. */
export async function fetchChromeUsers(id = 'oaoekkgkhmgdfnpmfkpphgiikliaicll') {
  try {
    const res = await fetch(`https://chromewebstore.google.com/detail/${id}`, {
      headers: { 'user-agent': CHROME_USER_AGENT, 'accept-language': 'en' },
      signal: AbortSignal.timeout(20_000),
    });
    if (!res.ok) return null;
    return parseChromeUsers(await res.text());
  } catch {
    return null;
  }
}

/**
 * Microsoft Store — OAuth2 client-credentials token, then a paginated
 * analytics GET. `null` when any of the three secrets is missing (collectStoreCounts
 * logs the one-line "unavailable" note), so an unconfigured store degrades
 * exactly like an unreachable one. Never logs the token, the secret, a URL
 * that carries them, or a response body.
 */
export async function fetchMsStoreAcquisitions(env = process.env) {
  const { MSSTORE_TENANT_ID, MSSTORE_CLIENT_ID, MSSTORE_CLIENT_SECRET } = env;
  if (!MSSTORE_TENANT_ID || !MSSTORE_CLIENT_ID || !MSSTORE_CLIENT_SECRET) {
    return null;
  }
  try {
    const tokenRes = await fetch(
      `https://login.microsoftonline.com/${MSSTORE_TENANT_ID}/oauth2/token`,
      {
        method: 'POST',
        headers: { 'content-type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({
          grant_type: 'client_credentials',
          client_id: MSSTORE_CLIENT_ID,
          client_secret: MSSTORE_CLIENT_SECRET,
          resource: 'https://manage.devcenter.microsoft.com',
        }),
        redirect: 'error',
        signal: AbortSignal.timeout(20_000),
      }
    );
    if (!tokenRes.ok) return null;
    const token = (await tokenRes.json())?.access_token;
    if (!token) return null;

    const today = new Date().toISOString().slice(0, 10);
    let url =
      'https://manage.devcenter.microsoft.com/v1.0/my/analytics/appacquisitions' +
      `?applicationId=${MS_STORE_APP_ID}&startDate=${MS_STORE_START_DATE}&endDate=${today}` +
      '&aggregationLevel=day&top=10000';
    const pages = [];
    // Hard cap: `top=10000` with no `groupby` returns rows disaggregated
    // across date x market x deviceType x ..., so page count only grows over
    // time. A runaway (or looping) @nextLink must not spin forever.
    const MAX_PAGES = 50;
    // One shared deadline for the whole pagination walk (the token POST keeps
    // its own 20s) — a slow upstream that keeps answering just under 20s per
    // page could otherwise page for a very long time.
    const deadline = AbortSignal.timeout(60_000);
    while (url && pages.length < MAX_PAGES) {
      const res = await fetch(url, {
        headers: { authorization: `Bearer ${token}` },
        redirect: 'error',
        signal: deadline,
      });
      // Fail loud, not quiet: a non-2xx mid-pagination means the sum so far
      // is partial. Returning it would silently publish a truncated count.
      if (!res.ok) return null;
      const page = await res.json();
      pages.push(page);
      if (page['@nextLink']) {
        // `@nextLink` is documented as relative to the analytics base path,
        // so resolve it against a base rather than fetching it as-is — and
        // pin the result to the real host, so the bearer token can never be
        // sent to another origin even if a response were ever compromised.
        const next = new URL(
          page['@nextLink'],
          'https://manage.devcenter.microsoft.com/v1.0/my/analytics/'
        );
        if (next.origin !== 'https://manage.devcenter.microsoft.com') return null;
        url = next.href;
      } else {
        url = null;
      }
    }
    // Hitting the page cap with a next link still pending means the sum is
    // incomplete — publish null, never a truncated number.
    return url ? null : sumAcquisitions(pages);
  } catch {
    return null;
  }
}

/**
 * Snap Store — `snapcraft metrics` via the CLI (no HTTP API for this figure).
 * `null` when `SNAPCRAFT_STORE_CREDENTIALS` is unset, matching the MS Store
 * fetcher's "unconfigured degrades like unreachable" contract.
 */
export async function fetchSnapInstalledBase(name = 'ai-job-hunter', env = process.env) {
  if (!env.SNAPCRAFT_STORE_CREDENTIALS) return null;
  try {
    // Minimal env only — never the whole process env, which on CI also
    // carries GITHUB_TOKEN and the MS Store secrets this command has no
    // business seeing.
    const minimalEnv = {
      PATH: env.PATH,
      HOME: env.HOME,
      SNAPCRAFT_STORE_CREDENTIALS: env.SNAPCRAFT_STORE_CREDENTIALS,
    };
    const { stdout } = await execFileAsync(
      'snapcraft',
      ['metrics', name, '--name', 'installed_base_by_channel', '--format', 'json'],
      { timeout: 60_000, env: minimalEnv }
    );
    return snapInstalledBase(JSON.parse(stdout));
  } catch (err) {
    // Only the exit code — never a stderr snippet, even truncated: a
    // truncated prefix can still defeat GitHub's exact-match secret masking.
    const code = err?.code ?? 'unknown';
    process.stderr.write(`store-counts: snap metrics failed (exit ${code})\n`);
    return null;
  }
}

/**
 * Runs all four fetchers concurrently and reports one line per store on
 * stderr. `Promise.allSettled` rather than `Promise.all`: every fetcher above
 * already catches its own errors and resolves to `null`, but this stays
 * correct even if one of them doesn't.
 */
export async function collectStoreCounts(env = process.env) {
  const stores = [
    ['msStore', fetchMsStoreAcquisitions(env)],
    ['snap', fetchSnapInstalledBase('ai-job-hunter', env)],
    ['chrome', fetchChromeUsers()],
    ['firefox', fetchFirefoxUsers()],
  ];
  const settled = await Promise.allSettled(stores.map(([, p]) => p));

  const result = {};
  settled.forEach((outcome, i) => {
    const [store] = stores[i];
    const value = outcome.status === 'fulfilled' ? outcome.value : null;
    const usable = typeof value === 'number' && Number.isFinite(value);
    result[store] = usable ? value : null;
    if (usable) {
      process.stderr.write(`store-counts: ${store} = ${value}\n`);
    } else {
      const reason = outcome.status === 'rejected' ? (outcome.reason?.name ?? 'error') : 'no data';
      process.stderr.write(`store-counts: ${store} unavailable (${reason})\n`);
    }
  });
  return result;
}
