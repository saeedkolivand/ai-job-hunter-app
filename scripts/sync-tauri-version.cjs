#!/usr/bin/env node
/**
 * sync-tauri-version.js
 *
 * Syncs the semantic-release version into all Tauri version fields.
 *
 * Usage:
 *   node scripts/sync-tauri-version.js <version>
 *
 * The updater public key is NOT touched here. It is committed directly in
 * tauri.conf.json (a public key is not a secret) so it is the single source of
 * truth and cannot silently diverge from the signing key. See
 * docs/DEPLOYMENT.md (Updater signing keys).
 *
 * Files updated:
 *   apps/tauri/src-tauri/tauri.conf.json  — version
 *   apps/tauri/src-tauri/Cargo.toml       — [package] version
 *   apps/tauri/src-tauri/Cargo.lock       — ajh-tauri package entry (kept in lockstep)
 *   apps/tauri/package.json               — version
 *   package.json                          — version
 *   README.md                             — static release badge version
 */

'use strict';

const fs = require('fs');
const path = require('path');

const version = process.argv[2];
if (!version) {
  console.error('Usage: node scripts/sync-tauri-version.js <version>');
  process.exit(1);
}

const root = path.resolve(__dirname, '..');

// ── tauri.conf.json ────────────────────────────────────────────────────────

const confPath = path.join(root, 'apps/tauri/src-tauri/tauri.conf.json');
const conf = JSON.parse(fs.readFileSync(confPath, 'utf8'));
conf.version = version;
fs.writeFileSync(confPath, JSON.stringify(conf, null, 2) + '\n');
console.log(`tauri.conf.json  → version=${version}`);

// ── Cargo.toml ─────────────────────────────────────────────────────────────

const cargoPath = path.join(root, 'apps/tauri/src-tauri/Cargo.toml');
let cargo = fs.readFileSync(cargoPath, 'utf8');
cargo = cargo.replace(/^version = ".*"$/m, `version = "${version}"`);
fs.writeFileSync(cargoPath, cargo);
console.log(`Cargo.toml       → version=${version}`);

// ── Cargo.lock ───────────────────────────────────────────────────────────────
//
// Keep the `ajh-tauri` package entry's version in lockstep with Cargo.toml.
// Without this the lockfile drifts: Cargo.toml is bumped here but the next
// local `cargo` run rewrites the stale Cargo.lock version line, leaving a
// recurring 1-line dirty diff in every working tree after a release. We only
// touch our own workspace member's version (a path member with no checksum),
// which is exactly what cargo would write — so no resolution/build change.

const lockPath = path.join(root, 'apps/tauri/src-tauri/Cargo.lock');
let lock = fs.readFileSync(lockPath, 'utf8');
const lockRe = /(name = "ajh-tauri"\r?\nversion = ")[^"]*(")/;
if (lockRe.test(lock)) {
  lock = lock.replace(lockRe, `$1${version}$2`);
  fs.writeFileSync(lockPath, lock);
  console.log(`Cargo.lock       → ajh-tauri version=${version}`);
} else {
  console.warn('Cargo.lock       → ajh-tauri package entry not found (skipped)');
}

// ── apps/tauri/package.json ────────────────────────────────────────────────

const pkgPath = path.join(root, 'apps/tauri/package.json');
const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
pkg.version = version;
fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n');
console.log(`apps/tauri/package.json → version=${version}`);

// ── root package.json ───────────────────────────────────────────────────────

const rootPkgPath = path.join(root, 'package.json');
const rootPkg = JSON.parse(fs.readFileSync(rootPkgPath, 'utf8'));
rootPkg.version = version;
fs.writeFileSync(rootPkgPath, JSON.stringify(rootPkg, null, 2) + '\n');
console.log(`package.json → version=${version}`);

// ── README.md release badge ──────────────────────────────────────────────────
//
// The release badge is a STATIC shields.io badge (img.shields.io/badge/...) so
// it never calls the GitHub API — the dynamic /github/v/release endpoint
// intermittently renders "Unable to select next GitHub token from pool" when
// shields' shared token pool is rate-limited. We keep it accurate by rewriting
// the version here on every release. Matches: release-v<version>-<6-hex-color>.

const readmePath = path.join(root, 'README.md');
const readme = fs.readFileSync(readmePath, 'utf8');
const badgeRe = /(img\.shields\.io\/badge\/release-v)[0-9][0-9.]*(-[0-9a-fA-F]{6})/;
if (badgeRe.test(readme)) {
  fs.writeFileSync(readmePath, readme.replace(badgeRe, `$1${version}$2`));
  console.log(`README.md → release badge v${version}`);
} else {
  console.warn('README.md → release badge not found (skipped)');
}
