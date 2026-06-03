#!/usr/bin/env node
/*
 * Sync the Homebrew cask (Casks/ai-job-hunter.rb) to a published release's
 * macOS .dmg assets: rewrites `version` and pins both per-arch `sha256` values.
 *
 *   node scripts/sync-cask.cjs <version> <armSha256> <intelSha256>
 *
 * Run by the release pipeline's `update-cask` job AFTER the installer build,
 * which is the only point where the dmgs — and therefore their hashes — exist.
 * It is NOT part of `sync-tauri-version.cjs`: that runs on every release push,
 * before (and usually without) a build, so there would be no dmgs to hash and
 * bumping the cask version there would point it at assets that never ship.
 */
const fs = require('node:fs');
const path = require('node:path');

const [version, armSha, intelSha] = process.argv.slice(2);
if (!version || !armSha || !intelSha) {
  console.error('usage: sync-cask.cjs <version> <armSha256> <intelSha256>');
  process.exit(1);
}

const SHA_RE = /^[0-9a-f]{64}$/;
for (const [name, value] of [
  ['arm', armSha],
  ['intel', intelSha],
]) {
  if (!SHA_RE.test(value)) {
    console.error(`invalid ${name} sha256 (expected 64 lowercase hex chars): ${value}`);
    process.exit(1);
  }
}

const caskPath = path.join(__dirname, '..', 'Casks', 'ai-job-hunter.rb');
const before = fs.readFileSync(caskPath, 'utf8');

const versionRe = /^(\s*version\s+)"[^"]*"/m;
// The sha256 stanza — whether it's `:no_check` or an existing two-line
// arm:/intel: block — gets rewritten to verified per-arch hashes.
const sha256Re = /^(\s*)sha256\b[^\n]*(\n[^\n]*intel:[^\n]*)?/m;

if (!versionRe.test(before) || !sha256Re.test(before)) {
  console.error('cask format unexpected — version/sha256 stanza not found');
  process.exit(1);
}

const cask = before
  .replace(versionRe, `$1"${version}"`)
  .replace(sha256Re, `$1sha256 arm:   "${armSha}",\n$1       intel: "${intelSha}"`);

if (cask === before) {
  console.log(`Cask already at v${version} with these hashes — no change.`);
  process.exit(0);
}

fs.writeFileSync(caskPath, cask);
console.log(`Cask synced to v${version} (arm + intel sha256 pinned).`);
