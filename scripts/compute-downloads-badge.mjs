// Computes an accurate "real installer" downloads count for the README badge.
//
// Why not shields' github/downloads/<repo>/total? That badge sums EVERY release
// asset's download_count, which for a Tauri auto-updating app is dominated by
// updater-channel traffic — NOT fresh installs:
//   - latest.json           → pure auto-update polling (hundreds of hits/day)
//   - *.sig sidecars        → updater signature fetches
//   - *.app.tar.gz          → macOS Tauri update payloads (in-place updates)
//   - *.nsis.zip            → Windows Tauri update payloads
//   - extension *.zip        → browser-store packaging bundles, not app installs
//   - any other *.zip/*.tar.gz → update/extension artifacts, never a fresh install
// Counting ONLY OS installer files gives the real "downloaded the app to install
// it" number. Everything else is updater-channel or extension-store noise.
//
// Output: badge-out/downloads.json — a Shields endpoint-badge payload, published
// by .github/workflows/downloads-badge.yml to the orphan `badges` branch.
// Run locally: GITHUB_TOKEN=$(gh auth token) node scripts/compute-downloads-badge.mjs

import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = 'saeedkolivand/ai-job-hunter-app';

// Real OS installers only (case-insensitive). Excludes .sig, .json (latest.json),
// .app.tar.gz / .nsis.zip / any .zip / any .tar.gz — those are updater or
// extension-store artifacts, not fresh installs.
const INSTALLER_RE = /\.(dmg|exe|msi|appimage|deb|rpm)$/i;

const token = process.env.GITHUB_TOKEN || process.env.GH_TOKEN;

async function fetchAllReleases() {
  const headers = {
    accept: 'application/vnd.github+json',
    'user-agent': 'ai-job-hunter-downloads-badge',
    'x-github-api-version': '2022-11-28',
  };
  if (token) headers.authorization = `Bearer ${token}`;

  const releases = [];
  for (let page = 1; ; page++) {
    const url = `https://api.github.com/repos/${REPO}/releases?per_page=100&page=${page}`;
    const res = await fetch(url, { headers });
    if (!res.ok) {
      throw new Error(`GitHub API ${res.status} ${res.statusText}: ${await res.text()}`);
    }
    const batch = await res.json();
    releases.push(...batch);
    if (batch.length < 100) break;
  }
  return releases;
}

// Shields does NOT humanize an endpoint `message`, so do it here:
// <1000 verbatim, >=1000 as "X.Yk".
function humanize(n) {
  return n < 1000 ? String(n) : `${(n / 1000).toFixed(1)}k`;
}

const releases = await fetchAllReleases();

let total = 0;
for (const release of releases) {
  for (const asset of release.assets ?? []) {
    if (INSTALLER_RE.test(asset.name)) total += asset.download_count ?? 0;
  }
}

const badge = {
  schemaVersion: 1,
  label: 'downloads',
  message: humanize(total),
  color: 'e24b4a',
};

const outDir = join(dirname(fileURLToPath(import.meta.url)), '..', 'badge-out');
mkdirSync(outDir, { recursive: true });
writeFileSync(join(outDir, 'downloads.json'), `${JSON.stringify(badge, null, 2)}\n`);

process.stderr.write(
  `installer downloads: ${total} (badge message: ${badge.message}) -> badge-out/downloads.json\n`
);
