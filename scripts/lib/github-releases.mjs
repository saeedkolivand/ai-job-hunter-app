// Shared GitHub release helpers for the downloads badge and the downloads chart.
//
// Both consumers must agree on what counts as a "download", or the badge and the
// chart under it would print different numbers for the same day. That agreement
// lives here, once.

/**
 * Real OS installers only (case-insensitive). Excludes `.sig`, `.json`
 * (latest.json), `.app.tar.gz` / `.nsis.zip` / any `.zip` / any `.tar.gz` —
 * those are updater-channel or extension-store artifacts, not fresh installs.
 * See `scripts/build-repo-charts.mjs` for why the stock shields
 * `github/downloads/<repo>/total` badge is updater-inflated.
 */
export const INSTALLER_RE = /\.(dmg|exe|msi|appimage|deb|rpm)$/i;

export const DEFAULT_REPO = process.env.GITHUB_REPOSITORY || 'saeedkolivand/ai-job-hunter-app';

function headers() {
  const token = process.env.GITHUB_TOKEN || process.env.GH_TOKEN;
  const h = {
    accept: 'application/vnd.github+json',
    'user-agent': 'ai-job-hunter-repo-charts',
    'x-github-api-version': '2022-11-28',
  };
  if (token) h.authorization = `Bearer ${token}`;
  return h;
}

/** Every release, following pagination to the end. Throws on any non-2xx. */
export async function fetchAllReleases(repo = DEFAULT_REPO) {
  const releases = [];
  for (let page = 1; ; page++) {
    const url = `https://api.github.com/repos/${repo}/releases?per_page=100&page=${page}`;
    const res = await fetch(url, { headers: headers() });
    if (!res.ok) {
      throw new Error(`GitHub API ${res.status} ${res.statusText}: ${await res.text()}`);
    }
    const batch = await res.json();
    releases.push(...batch);
    if (batch.length < 100) break;
  }
  return releases;
}

/** Installer-only download total across every asset of every given release. */
export function installerDownloads(releases) {
  let total = 0;
  for (const release of releases) {
    for (const asset of release.assets ?? []) {
      if (INSTALLER_RE.test(asset.name)) total += asset.download_count ?? 0;
    }
  }
  return total;
}

/**
 * Stargazers with their `starred_at` timestamps.
 *
 * Needs the `star+json` media type — the default representation omits
 * `starred_at` entirely. Since GitHub's 2026-06-30 restriction this endpoint
 * only answers for a repo's own admins/collaborators, which is exactly why this
 * runs in-repo under `GITHUB_TOKEN` instead of through a third-party service.
 */
export async function fetchStargazers(repo = DEFAULT_REPO) {
  const stars = [];
  for (let page = 1; ; page++) {
    const url = `https://api.github.com/repos/${repo}/stargazers?per_page=100&page=${page}`;
    const res = await fetch(url, {
      headers: { ...headers(), accept: 'application/vnd.github.star+json' },
    });
    if (!res.ok) {
      throw new Error(`GitHub stargazers API ${res.status} ${res.statusText}: ${await res.text()}`);
    }
    const batch = await res.json();
    stars.push(...batch);
    if (batch.length < 100) break;
    // The stargazers endpoint hard-stops at 400 pages and returns an empty list
    // beyond it rather than an error, so page 400 coming back FULL means the
    // repo has more stars than this endpoint will ever hand over. Throw rather
    // than break: breaking would return a silently truncated 40,000 and publish
    // a star chart that is wrong without saying so. Same rule as the history
    // reader in build-repo-charts.mjs — never publish reduced data quietly.
    if (page >= 400) {
      throw new Error(
        'stargazers pagination limit reached (40,000) — the chart would be truncated; ' +
          'switch to the GraphQL API before publishing again'
      );
    }
  }
  return stars;
}
