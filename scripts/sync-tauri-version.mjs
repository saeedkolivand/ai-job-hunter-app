#!/usr/bin/env node
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '..');
const version = process.argv[2];

if (!version) {
  console.error('Usage: sync-tauri-version.mjs <version>');
  process.exit(1);
}

// apps/tauri/package.json
const pkgPath = resolve(root, 'apps/tauri/package.json');
const pkg = JSON.parse(readFileSync(pkgPath, 'utf8'));
pkg.version = version;
writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n');
console.log(`updated apps/tauri/package.json → ${version}`);

// apps/tauri/src-tauri/tauri.conf.json
const confPath = resolve(root, 'apps/tauri/src-tauri/tauri.conf.json');
const conf = JSON.parse(readFileSync(confPath, 'utf8'));
conf.version = version;
// Inject public key from environment variable (CI) or keep existing
const publicKey = process.env.TAURI_SIGNING_PUBLIC_KEY;
if (publicKey) {
  conf.plugins.updater.pubkey = publicKey;
  console.log(`updated apps/tauri/src-tauri/tauri.conf.json pubkey`);
}
writeFileSync(confPath, JSON.stringify(conf, null, 2) + '\n');
console.log(`updated apps/tauri/src-tauri/tauri.conf.json → ${version}`);

// apps/tauri/src-tauri/Cargo.toml (simple line replacement)
const cargoPath = resolve(root, 'apps/tauri/src-tauri/Cargo.toml');
const cargo = readFileSync(cargoPath, 'utf8');
const updated = cargo.replace(/^version = ".*"/m, `version = "${version}"`);
writeFileSync(cargoPath, updated);
console.log(`updated apps/tauri/src-tauri/Cargo.toml → ${version}`);
