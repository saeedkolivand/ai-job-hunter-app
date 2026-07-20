#!/usr/bin/env node
/*
 * Sync the landing /download version seam to a published release. Writes the
 * per-platform installer URLs + version into apps/landing/src/data/version.json;
 * the Next /download route imports that JSON at build time (and swaps to a newer
 * GitHub release at runtime via DownloadFreshness). No HTML surgery — the page
 * markup is owned by the port (apps/landing/src/app/download).
 *
 *   node scripts/sync-download-page.cjs <version>     # e.g. 0.103.0
 *
 * CLI contract unchanged (single <version> arg) so release.yml's
 * `update-download-page` job invokes it exactly as before. Run AFTER the
 * installer build (the versioned assets the URLs point to must exist on the
 * GitHub Release). The commit of version.json to main re-triggers the Pages
 * deploy (pages.yml watches apps/landing/**), so the live site updates.
 *
 * The asset filenames mirror the release notes Downloads table in
 * .github/workflows/release.yml — keep the two in sync if either changes.
 * KEEP `buildInstallers` IN SYNC with apps/landing/src/lib/version.ts.
 */
const fs = require('node:fs');
const path = require('node:path');

const REPO = 'https://github.com/saeedkolivand/ai-job-hunter-app';

const [version] = process.argv.slice(2);
if (!version) {
  console.error('usage: sync-download-page.cjs <version>');
  process.exit(1);
}

// Mirrors the version validation in release.yml's "Resolve Version" step.
const VERSION_RE = /^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$/;
if (!VERSION_RE.test(version)) {
  console.error(`invalid version (expected e.g. 0.103.0): ${version}`);
  process.exit(1);
}

/** Per-OS GitHub Release asset URLs pinned to `v`. Mirrors version.ts. */
function buildInstallers(v) {
  const base = `${REPO}/releases/download/v${v}`;
  return {
    macArm: `${base}/macos-AI-Job-Hunter_${v}_aarch64-apple-silicon.dmg`,
    macIntel: `${base}/macos-AI-Job-Hunter_${v}_x64-intel.dmg`,
    winExe: `${base}/windows-AI-Job-Hunter_${v}_x64-setup.exe`,
    winMsi: `${base}/windows-AI-Job-Hunter_${v}_x64_en-US.msi`,
    linuxAppImage: `${base}/linux-AI-Job-Hunter_${v}_amd64.AppImage`,
    linuxDeb: `${base}/linux-AI-Job-Hunter_${v}_amd64.deb`,
    linuxRpm: `${base}/linux-AI-Job-Hunter-${v}-1.x86_64.rpm`,
  };
}

const jsonPath = path.join(__dirname, '..', 'apps', 'landing', 'src', 'data', 'version.json');
const before = fs.existsSync(jsonPath) ? fs.readFileSync(jsonPath, 'utf8') : '';
let prev;
try {
  prev = JSON.parse(before);
} catch {
  prev = {};
}

const next = {
  version,
  // Preserve the timestamp when the version is unchanged (avoids a no-op diff);
  // otherwise stamp now.
  releasedAt: prev.version === version ? (prev.releasedAt ?? null) : new Date().toISOString(),
  installers: buildInstallers(version),
};

const after = `${JSON.stringify(next, null, 2)}\n`;
if (after === before) {
  console.log(`version.json already pinned to v${version} — no change.`);
  process.exit(0);
}

fs.writeFileSync(jsonPath, after);
console.log(`version.json synced to v${version} (macOS / Windows / Linux installer URLs pinned).`);
