#!/usr/bin/env node
/*
 * Sync the Snapcraft manifest's `version` field to a published release.
 *
 *   node scripts/sync-snapcraft.cjs <version>
 *
 * Run by the release pipeline's `publish-snap` job before `snapcraft pack`.
 * Mirrors scripts/sync-cask.cjs's job for the Homebrew cask — same reason it
 * isn't part of sync-tauri-version.cjs: this only makes sense once a release
 * (and its .deb) actually exists to pack.
 */
const fs = require('node:fs');
const path = require('node:path');

const [version] = process.argv.slice(2);
if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
  console.error('usage: sync-snapcraft.cjs <version>  (e.g. 0.152.0)');
  process.exit(1);
}

const manifestPath = path.join(
  __dirname,
  '..',
  'apps',
  'desktop',
  'src-tauri',
  'linux',
  'snap',
  'snapcraft.yaml'
);
const before = fs.readFileSync(manifestPath, 'utf8');

const versionRe = /^(version:\s*)'[^']*'/m;
if (!versionRe.test(before)) {
  console.error('snapcraft.yaml format unexpected — version field not found');
  process.exit(1);
}

const manifest = before.replace(versionRe, `$1'${version}'`);

if (manifest === before) {
  console.log(`snapcraft.yaml already at v${version} — no change.`);
  process.exit(0);
}

fs.writeFileSync(manifestPath, manifest);
console.log(`snapcraft.yaml synced to v${version}.`);
