#!/usr/bin/env node
/**
 * verify-updater-key.cjs
 *
 * CI guard against a broken auto-updater. It proves that the public key
 * committed in tauri.conf.json (the key shipped apps verify updates with) is
 * the public half of the private key that just signed the release artifacts.
 *
 * If they diverge, every shipped update fails minisign verification at download
 * time ("invalid encoding in minisign data" / signature errors) and the build
 * is aborted before anything is published.
 *
 * Usage:
 *   node scripts/verify-updater-key.cjs [sigFileOrDir ...]
 *
 * With no arguments it recursively scans the Tauri bundle output for *.sig
 * files. Pass explicit paths to narrow the check.
 *
 * Exit codes: 0 = all signatures match the committed key · 1 = mismatch / no
 * signatures found / unreadable key.
 */

'use strict';

const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..');
const CONF_PATH = path.join(root, 'apps/desktop/src-tauri/tauri.conf.json');
const DEFAULT_SCAN_DIR = path.join(root, 'apps/desktop/src-tauri/target');

// Directories under target/ that are large and never hold updater bundles.
const SKIP_DIRS = new Set([
  'deps',
  'incremental',
  '.fingerprint',
  'build',
  'examples',
  '.cargo',
  'node_modules',
]);

/**
 * Extract the 8-byte minisign key id from a minisign blob.
 * Accepts either the raw two/four-line text or its base64 wrapping (the form
 * Tauri writes into .sig files and the updater `pubkey` field).
 */
function keyIdFromMinisign(value, { label }) {
  let text = value.trim();
  if (!text.includes('untrusted comment')) {
    text = Buffer.from(text, 'base64').toString('utf8');
  }
  const lines = text.split(/\r?\n/).filter(Boolean);
  if (lines.length < 2) {
    throw new Error(`${label}: not a valid minisign blob (missing key line)`);
  }
  const raw = Buffer.from(lines[1].trim(), 'base64');
  // layout: 2-byte signature algorithm + 8-byte key id + payload
  if (raw.length < 10) {
    throw new Error(`${label}: minisign payload too short (got ${raw.length} bytes)`);
  }
  const keyId = Buffer.from(raw.subarray(2, 10)).reverse().toString('hex').toUpperCase();
  return { keyId, algo: raw.subarray(0, 2).toString('latin1') };
}

function readCommittedPubKeyId() {
  const conf = JSON.parse(fs.readFileSync(CONF_PATH, 'utf8'));
  const pubkey = conf?.plugins?.updater?.pubkey;
  if (!pubkey || typeof pubkey !== 'string') {
    throw new Error('tauri.conf.json has no plugins.updater.pubkey');
  }
  return keyIdFromMinisign(pubkey, { label: 'tauri.conf.json pubkey' }).keyId;
}

function collectSigFiles(target) {
  const stat = fs.statSync(target);
  if (stat.isFile()) return target.endsWith('.sig') ? [target] : [];

  const found = [];
  const stack = [target];
  while (stack.length) {
    const dir = stack.pop();
    let entries;
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const entry of entries) {
      if (entry.isDirectory()) {
        if (!SKIP_DIRS.has(entry.name)) stack.push(path.join(dir, entry.name));
      } else if (entry.name.endsWith('.sig')) {
        found.push(path.join(dir, entry.name));
      }
    }
  }
  return found;
}

function main() {
  const args = process.argv.slice(2);
  const targets = args.length ? args : [DEFAULT_SCAN_DIR];

  let expectedKeyId;
  try {
    expectedKeyId = readCommittedPubKeyId();
  } catch (err) {
    console.error(`✗ Could not read committed updater public key: ${err.message}`);
    process.exit(1);
  }

  const sigFiles = [...new Set(targets.flatMap((t) => collectSigFiles(t)))];
  if (sigFiles.length === 0) {
    console.error('✗ No .sig files found to verify.');
    console.error(`  Searched: ${targets.join(', ')}`);
    process.exit(1);
  }

  console.log(`Committed updater public key id: ${expectedKeyId}`);
  console.log(`Checking ${sigFiles.length} signature file(s)…`);

  const mismatches = [];
  for (const sig of sigFiles) {
    const rel = path.relative(root, sig);
    try {
      const { keyId } = keyIdFromMinisign(fs.readFileSync(sig, 'utf8'), { label: rel });
      if (keyId === expectedKeyId) {
        console.log(`  ✓ ${rel} (${keyId})`);
      } else {
        console.log(`  ✗ ${rel} (${keyId})`);
        mismatches.push({ rel, keyId });
      }
    } catch (err) {
      console.log(`  ✗ ${rel} — ${err.message}`);
      mismatches.push({ rel, keyId: 'unreadable' });
    }
  }

  if (mismatches.length) {
    console.error('');
    console.error('✗ Updater key mismatch — shipped apps would reject these updates.');
    console.error(`  Committed pubkey id: ${expectedKeyId}`);
    for (const m of mismatches) console.error(`  Signed with:         ${m.keyId}  (${m.rel})`);
    console.error('');
    console.error('  The TAURI_SIGNING_PRIVATE_KEY used to sign must match the pubkey');
    console.error('  in tauri.conf.json. See docs/DEPLOYMENT.md (Updater signing keys).');
    process.exit(1);
  }

  console.log('✓ All signatures match the committed updater public key.');
}

main();
