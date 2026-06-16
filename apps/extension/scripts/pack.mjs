/**
 * Pack the built extension into store-upload zips.
 *
 * Produces three archives in `dist/` (gitignored) for the current package
 * version:
 *   - <name>-chrome-<v>.zip   → Chrome Web Store    (manifest.json at the root)
 *   - <name>-firefox-<v>.zip  → AMO (the add-on)    (manifest.json at the root)
 *   - <name>-firefox-source-<v>.zip → AMO source code (AMO requires the source
 *     for build-tool/minified submissions; a clean `git archive` of the tracked
 *     monorepo at HEAD, so it reproduces with `pnpm install && pnpm -F
 *     @ajh/extension build`).
 *
 * Run via `pnpm -F @ajh/extension pack` (which builds first). Assumes the build
 * already populated `dist/chrome` and `dist/firefox`.
 *
 * ponytail: shells out to the OS zip tool (PowerShell Compress-Archive on
 * Windows, `zip` elsewhere) + `git archive` instead of pulling an archiver
 * dependency — Node has no built-in zip writer and these are always present on a
 * dev box. Swap in a JS archiver only if this ever needs to run where neither is.
 */

import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, rmSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const NAME = 'ai-job-hunter-job-importer';

const extRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const dist = join(extRoot, 'dist');
const version = JSON.parse(readFileSync(join(extRoot, 'package.json'), 'utf8')).version;

/** Zip the CONTENTS of `srcDir` (so manifest.json lands at the archive root). */
function zipDirContents(srcDir, outZip) {
  if (!existsSync(srcDir)) {
    throw new Error(`missing build output: ${srcDir} — run \`pnpm -F @ajh/extension build\` first`);
  }
  if (process.platform === 'win32') {
    execFileSync(
      'powershell',
      [
        '-NoProfile',
        '-NonInteractive',
        '-Command',
        `Compress-Archive -Path '${join(srcDir, '*')}' -DestinationPath '${outZip}' -Force`,
      ],
      { stdio: 'inherit' }
    );
  } else {
    // `zip` archives relative to its cwd, so run it inside srcDir → files at root.
    execFileSync('zip', ['-r', '-q', outZip, '.'], { cwd: srcDir, stdio: 'inherit' });
  }
}

// Clean stale zips so a re-pack never leaves an old version behind.
for (const f of readdirSync(dist)) {
  if (f.endsWith('.zip')) rmSync(join(dist, f));
}

const outputs = {
  chrome: join(dist, `${NAME}-chrome-${version}.zip`),
  firefox: join(dist, `${NAME}-firefox-${version}.zip`),
  source: join(dist, `${NAME}-firefox-source-${version}.zip`),
};

zipDirContents(join(dist, 'chrome'), outputs.chrome);
zipDirContents(join(dist, 'firefox'), outputs.firefox);

// Source archive: tracked files at HEAD (no node_modules/dist/secrets), from the
// repo root so workspace deps are included and the build reproduces.
const repoRoot = execFileSync('git', ['rev-parse', '--show-toplevel']).toString().trim();
execFileSync('git', ['archive', '--format=zip', '-o', outputs.source, 'HEAD'], { cwd: repoRoot });

console.log(`\nPacked @ajh/extension v${version} → apps/extension/dist:`);
for (const [label, p] of Object.entries(outputs)) {
  const kb = (statSync(p).size / 1024).toFixed(1);
  console.log(`  ${label.padEnd(8)} ${p.split(/[/\\]/).pop()}  (${kb} KB)`);
}
