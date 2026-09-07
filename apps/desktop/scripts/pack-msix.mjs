#!/usr/bin/env node
/**
 * Pack the built desktop app as an MSIX for the Microsoft Store.
 *
 * Input is the SAME `ajh-tauri.exe` the NSIS installer ships (Tauri has no
 * MSIX bundle target); this script only wraps it, so it must run AFTER
 * `tauri build`. The result is deliberately UNSIGNED — the Store signs the
 * package on submission — which is also why it is a workflow artifact rather
 * than a release asset (.github/workflows/release.yml).
 *
 * Package identity is not committed: `Name`/`Publisher`/`PublisherDisplayName`
 * are assigned by Partner Center and arrive as env vars (GitHub repository
 * variables in CI). See docs/DEPLOYMENT.md § Microsoft Store (MSIX).
 *
 * Usage: node apps/desktop/scripts/pack-msix.mjs
 *   env MSIX_IDENTITY_NAME, MSIX_PUBLISHER, MSIX_PUBLISHER_DISPLAY_NAME (required)
 *   env AJH_EXE    — override the staged executable (tests / local dry runs)
 *   env MAKEAPPX   — override makeappx.exe discovery
 */
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const APP_DIR = path.resolve(HERE, '..');
const TAURI_DIR = path.join(APP_DIR, 'src-tauri');
const REPO_ROOT = path.resolve(APP_DIR, '..', '..');

const MSIX_DIR = path.join(TAURI_DIR, 'windows', 'msix');
const OUT_DIR = path.join(TAURI_DIR, 'target', 'msix');
const STAGING_DIR = path.join(OUT_DIR, 'staging');

/** Identity values Partner Center owns, in the order they are reported missing. */
const IDENTITY_VARS = ['MSIX_IDENTITY_NAME', 'MSIX_PUBLISHER', 'MSIX_PUBLISHER_DISPLAY_NAME'];

/**
 * `x.y.z` → `x.y.z.0`. Store packages carry a four-part version whose
 * Revision must be 0 (Microsoft reserves it), and they have no notion of a
 * prerelease suffix — so anything but a plain three-part release version is a
 * hard error rather than a lossy coercion.
 */
export function toStoreVersion(version) {
  if (!/^\d+\.\d+\.\d+$/.test(String(version).trim())) {
    throw new Error(
      `MSIX needs a plain three-part release version, got "${version}". ` +
        'Store package versions are x.y.z.0 with no prerelease suffix.'
    );
  }
  return `${String(version).trim()}.0`;
}

/** XML-escape a value before it is substituted into the manifest. */
function xmlEscape(value) {
  return String(value)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

/**
 * Substitute `{{PLACEHOLDER}}`s in the manifest template. Throws if any
 * placeholder is left over — a manifest shipped with a literal `{{…}}` would
 * fail deep inside Partner Center rather than here.
 */
export function fillManifest(template, values) {
  let out = template;
  for (const [key, value] of Object.entries(values)) {
    out = out.split(`{{${key}}}`).join(xmlEscape(value));
  }
  const leftover = out.match(/\{\{[A-Z_]+\}\}/);
  if (leftover) {
    throw new Error(`AppxManifest.xml still contains an unfilled placeholder: ${leftover[0]}`);
  }
  return out;
}

/** Read + validate the Partner Center identity values from the environment. */
export function readIdentity(env = process.env) {
  for (const name of IDENTITY_VARS) {
    if (!env[name] || !String(env[name]).trim()) {
      throw new Error(
        `Missing ${name}. Set the MSIX_* repository variables (Partner Center → ` +
          'Product management → Product identity); see docs/DEPLOYMENT.md.'
      );
    }
  }
  const publisher = String(env.MSIX_PUBLISHER).trim();
  if (!publisher.startsWith('CN=')) {
    throw new Error(
      `MSIX_PUBLISHER must be the full subject Partner Center shows, starting with "CN=" — got "${publisher}".`
    );
  }
  return {
    IDENTITY_NAME: String(env.MSIX_IDENTITY_NAME).trim(),
    PUBLISHER: publisher,
    PUBLISHER_DISPLAY_NAME: String(env.MSIX_PUBLISHER_DISPLAY_NAME).trim(),
  };
}

/** Numeric comparison of SDK directory names (`10.0.26100.0`), newest first. */
function compareSdkVersions(a, b) {
  const pa = a.split('.').map(Number);
  const pb = b.split('.').map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i += 1) {
    const diff = (pb[i] ?? 0) - (pa[i] ?? 0);
    if (diff !== 0) return diff;
  }
  return 0;
}

/**
 * Locate `makeappx.exe`: the `MAKEAPPX` override first, otherwise the newest
 * Windows 10 SDK under either Program Files. Not bundled with anything the
 * repo installs — the GitHub Windows runner ships the SDK, a local machine
 * needs it from the Windows SDK installer.
 */
export function findMakeappx(env = process.env) {
  const override = env.MAKEAPPX && String(env.MAKEAPPX).trim();
  if (override) {
    if (!fs.existsSync(override)) throw new Error(`MAKEAPPX points at a missing file: ${override}`);
    return override;
  }
  const roots = [env['ProgramFiles(x86)'], env.ProgramFiles]
    .filter(Boolean)
    .map((root) => path.join(root, 'Windows Kits', '10', 'bin'));
  for (const root of roots) {
    if (!fs.existsSync(root)) continue;
    const versions = fs
      .readdirSync(root, { withFileTypes: true })
      .filter((entry) => entry.isDirectory() && entry.name.startsWith('10.'))
      .map((entry) => entry.name)
      .sort(compareSdkVersions);
    for (const version of versions) {
      const candidate = path.join(root, version, 'x64', 'makeappx.exe');
      if (fs.existsSync(candidate)) return candidate;
    }
  }
  throw new Error(
    'makeappx.exe not found. Install the Windows 10/11 SDK (App Certification Kit / MSIX ' +
      'packaging tools), or set MAKEAPPX to its full path.'
  );
}

/** Repo-relative path for logging — never print an absolute path. */
function rel(target) {
  return path.relative(REPO_ROOT, target).split(path.sep).join('/');
}

export function packMsix(env = process.env) {
  const identity = readIdentity(env);
  const { version } = JSON.parse(fs.readFileSync(path.join(TAURI_DIR, 'tauri.conf.json'), 'utf8'));
  const storeVersion = toStoreVersion(version);
  const makeappx = findMakeappx(env);

  const exe = env.AJH_EXE
    ? path.resolve(String(env.AJH_EXE))
    : path.join(TAURI_DIR, 'target', 'release', 'ajh-tauri.exe');
  if (!fs.existsSync(exe)) {
    throw new Error(`Built executable not found at ${rel(exe)} — run \`tauri build\` first.`);
  }

  // Rebuilt from scratch every run: a leftover file in the staging directory
  // would be packed into the submission without anyone noticing.
  fs.rmSync(STAGING_DIR, { recursive: true, force: true });
  fs.mkdirSync(STAGING_DIR, { recursive: true });
  fs.copyFileSync(exe, path.join(STAGING_DIR, 'ajh-tauri.exe'));
  fs.cpSync(path.join(MSIX_DIR, 'Assets'), path.join(STAGING_DIR, 'Assets'), { recursive: true });
  fs.writeFileSync(
    path.join(STAGING_DIR, 'AppxManifest.xml'),
    fillManifest(fs.readFileSync(path.join(MSIX_DIR, 'AppxManifest.xml'), 'utf8'), {
      ...identity,
      VERSION: storeVersion,
    }),
    'utf8'
  );

  const output = path.join(OUT_DIR, `AI-Job-Hunter_${storeVersion}_x64.msix`);
  execFileSync(makeappx, ['pack', '/o', '/d', STAGING_DIR, '/p', output], { stdio: 'inherit' });
  console.log(`MSIX written to ${rel(output)} (unsigned — the Microsoft Store signs it).`);
  return output;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    packMsix();
  } catch (error) {
    console.error(`pack-msix: ${error.message}`);
    process.exitCode = 1;
  }
}
