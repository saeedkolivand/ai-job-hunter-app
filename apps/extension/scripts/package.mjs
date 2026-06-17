// Store-ready zip packager for the AI Job Hunter browser extension.
//
// Zips the built dist/chrome and dist/firefox folders into upload-ready archives
// with manifest.json at the ZIP ROOT (Chrome Web Store / AMO require this — no
// nested <target>/ folder) using forward-slash separators.
//
// Zero-dependency, cross-platform: prefers the `zip` binary (CI/macOS/Linux),
// and falls back to PowerShell `Compress-Archive` on Windows when `zip` is
// missing (preferring pwsh 7, which writes forward-slash separators).
//
// Run: pnpm -F @ajh/extension package   (builds first, then zips)
//      node apps/extension/scripts/package.mjs   (zips an existing build)

import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { existsSync, rmSync, statSync } from 'node:fs';
import path from 'node:path';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const EXT_ROOT = path.resolve(__dirname, '..');
const REPO_ROOT = path.resolve(EXT_ROOT, '..', '..');
const DIST = path.join(EXT_ROOT, 'dist');

const { version } = createRequire(import.meta.url)('../package.json');
const TARGETS = ['chrome', 'firefox'];

// `zip` present? (CI/macOS/Linux). Detect via a cheap version probe.
const HAS_ZIP = spawnSync('zip', ['-v'], { stdio: 'ignore' }).status === 0;
const rel = (p) => path.relative(REPO_ROOT, p).split(path.sep).join('/');

function zipTarget(target) {
  const srcDir = path.join(DIST, target);
  if (!existsSync(path.join(srcDir, 'manifest.json'))) {
    console.error(
      `error: ${rel(srcDir)}/manifest.json not found — build first: pnpm -F @ajh/extension build`
    );
    process.exit(1);
  }
  const out = path.join(DIST, `ai-job-hunter-extension-${target}-${version}.zip`);
  rmSync(out, { force: true }); // avoid `zip` appending stale entries

  let res;
  if (HAS_ZIP) {
    // Run from inside the source dir so entries are root-relative (`.`).
    res = spawnSync('zip', ['-qr', out, '.'], { cwd: srcDir, stdio: 'inherit' });
  } else if (process.platform === 'win32') {
    // `<src>/*` makes entries root-relative; -Force overwrites. pwsh 7 writes
    // forward slashes; powershell 5.1 is the fallback.
    const cmd = `Compress-Archive -Path '${srcDir}/*' -DestinationPath '${out}' -Force`;
    const shell =
      spawnSync('pwsh', ['-v'], { stdio: 'ignore' }).status === 0 ? 'pwsh' : 'powershell';
    res = spawnSync(shell, ['-NoProfile', '-NonInteractive', '-Command', cmd], {
      stdio: 'inherit',
    });
  } else {
    console.error('error: `zip` binary is required to package on this platform');
    process.exit(1);
  }

  if (res.status !== 0) {
    console.error(`error: packaging ${target} failed (exit ${res.status})`);
    process.exit(res.status ?? 1);
  }
  const bytes = statSync(out).size;
  console.log(`packaged ${rel(out)}  ${bytes} bytes (${(bytes / 1024).toFixed(1)} kB)`);
}

for (const target of TARGETS) zipTarget(target);
