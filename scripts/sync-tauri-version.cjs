#!/usr/bin/env node
/**
 * sync-tauri-version.js
 *
 * Syncs the semantic-release version into all three Tauri version fields and
 * injects the minisign public key from the environment.
 *
 * Usage:
 *   node scripts/sync-tauri-version.js <version>
 *
 * Required env vars:
 *   TAURI_SIGNING_PUBLIC_KEY  — base64-encoded minisign public key
 *                               (from the TAURI_SIGNING_PUBLIC_KEY GitHub secret)
 *
 * Files updated:
 *   apps/tauri/src-tauri/tauri.conf.json  — version + plugins.updater.pubkey
 *   apps/tauri/src-tauri/Cargo.toml       — [package] version
 *   apps/tauri/package.json               — version
 */

'use strict';

const fs = require('fs');
const path = require('path');

const version = process.argv[2];
if (!version) {
  console.error('Usage: node scripts/sync-tauri-version.js <version>');
  process.exit(1);
}

const pubkey = process.env.TAURI_SIGNING_PUBLIC_KEY;
if (!pubkey) {
  console.error('TAURI_SIGNING_PUBLIC_KEY environment variable is required');
  process.exit(1);
}

const root = path.resolve(__dirname, '..');

// ── tauri.conf.json ────────────────────────────────────────────────────────

const confPath = path.join(root, 'apps/tauri/src-tauri/tauri.conf.json');
const conf = JSON.parse(fs.readFileSync(confPath, 'utf8'));
conf.version = version;
conf.plugins = conf.plugins ?? {};
conf.plugins.updater = conf.plugins.updater ?? {};
conf.plugins.updater.pubkey = pubkey;
fs.writeFileSync(confPath, JSON.stringify(conf, null, 2) + '\n');
console.log(`tauri.conf.json  → version=${version}, pubkey injected`);

// ── Cargo.toml ─────────────────────────────────────────────────────────────

const cargoPath = path.join(root, 'apps/tauri/src-tauri/Cargo.toml');
let cargo = fs.readFileSync(cargoPath, 'utf8');
cargo = cargo.replace(/^version = ".*"$/m, `version = "${version}"`);
fs.writeFileSync(cargoPath, cargo);
console.log(`Cargo.toml       → version=${version}`);

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
