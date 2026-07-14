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
import { existsSync, readFileSync, rmSync, statSync } from 'node:fs';
import path from 'node:path';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const EXT_ROOT = path.resolve(__dirname, '..');
const REPO_ROOT = path.resolve(EXT_ROOT, '..', '..');
const DIST = path.join(EXT_ROOT, 'dist');

const { version } = createRequire(import.meta.url)('../package.json');
const TARGETS = ['chrome', 'firefox'];

// `fill.js`/`capture.js` are injected via
// `chrome.scripting.executeScript({ files: [...] })` as CLASSIC scripts — no
// ES module support. `vite.config.ts`'s `injectedEntries` plugin builds them
// each in an isolated Rollup pass specifically so no `import`/`export`
// statement ever leaks in (see field-signal.ts's header comment); this is the
// automated guard that invariant doesn't silently regress.
const INJECTED_CLASSIC_SCRIPTS = ['fill.js', 'capture.js'];
const IMPORT_EXPORT_TOKEN_RE = /\b(?:import|export)\b/;
// A minifier can emit `import`/`export` mid-line (e.g. `...;import{x}from"y";...`),
// so a line-anchored `^\s*` check misses it. Strip string/template literals and
// comments first (so the bare word "import"/"export" inside quoted text or a
// comment doesn't false-positive), then look for the token anywhere in what's
// left — that also catches a dynamic `import(...)`, which is equally illegal in
// a classic `executeScript` bundle.
function containsImportOrExportStatement(src) {
  const stripped = src
    .replace(/\/\*[\s\S]*?\*\//g, ' ') // block comments
    .replace(/\/\/[^\n]*/g, ' ') // line comments
    .replace(/`(?:\\.|[^`\\])*`/g, ' ') // template literals
    .replace(/"(?:\\.|[^"\\])*"/g, ' ') // double-quoted strings
    .replace(/'(?:\\.|[^'\\])*'/g, ' '); // single-quoted strings
  return IMPORT_EXPORT_TOKEN_RE.test(stripped);
}

// `zip` present? (CI/macOS/Linux). Detect via a cheap version probe.
const HAS_ZIP = spawnSync('zip', ['-v'], { stdio: 'ignore' }).status === 0;
const rel = (p) => path.relative(REPO_ROOT, p).split(path.sep).join('/');

function assertClassicScripts(srcDir) {
  for (const file of INJECTED_CLASSIC_SCRIPTS) {
    const filePath = path.join(srcDir, file);
    if (!existsSync(filePath)) {
      console.error(
        `error: ${rel(filePath)} not found — build first: pnpm -F @ajh/extension build`
      );
      process.exit(1);
    }
    if (containsImportOrExportStatement(readFileSync(filePath, 'utf8'))) {
      console.error(
        `error: ${rel(filePath)} contains an import/export statement — it must be a classic ` +
          `script (chrome.scripting.executeScript({ files: [...] }) can't load ES modules). ` +
          `The injectedEntries isolated-build guarantee in vite.config.ts has regressed.`
      );
      process.exit(1);
    }
  }
}

function zipTarget(target) {
  const srcDir = path.join(DIST, target);
  if (!existsSync(path.join(srcDir, 'manifest.json'))) {
    console.error(
      `error: ${rel(srcDir)}/manifest.json not found — build first: pnpm -F @ajh/extension build`
    );
    process.exit(1);
  }
  assertClassicScripts(srcDir);
  const out = path.join(DIST, `ai-job-hunter-extension-${target}-${version}.zip`);
  rmSync(out, { force: true }); // avoid `zip` appending stale entries

  let res;
  if (HAS_ZIP) {
    // Run from inside the source dir so entries are root-relative (`.`).
    res = spawnSync('zip', ['-qr', out, '.'], { cwd: srcDir, stdio: 'inherit' });
  } else if (process.platform === 'win32') {
    // PowerShell escapes a literal single quote inside a single-quoted string by
    // doubling it; escape both paths so a directory containing a quote can't
    // break the command. `<src>/*` makes entries root-relative; -Force overwrites.
    // pwsh 7 writes forward slashes; powershell 5.1 is the fallback.
    const psQuote = (p) => p.replace(/'/g, "''");
    const cmd = `Compress-Archive -Path '${psQuote(srcDir)}/*' -DestinationPath '${psQuote(out)}' -Force`;
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
