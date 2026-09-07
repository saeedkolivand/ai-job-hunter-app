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
import { Script } from 'node:vm';

import { INJECTED_SCRIPT_FILES } from '../injected-entries.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const EXT_ROOT = path.resolve(__dirname, '..');
const REPO_ROOT = path.resolve(EXT_ROOT, '..', '..');
const DIST = path.join(EXT_ROOT, 'dist');

const { version } = createRequire(import.meta.url)('../package.json');
const TARGETS = ['chrome', 'firefox'];

// Every entry in INJECTED_SCRIPT_FILES is injected via
// `chrome.scripting.executeScript({ files: [...] })` as a CLASSIC script — no ES
// module support. `vite.config.mts`'s `injectedEntries` plugin builds them each
// in an isolated Rollup pass specifically so no `import`/`export` statement ever
// leaks in (see field-signal.ts's header comment); this is the automated guard
// that invariant doesn't silently regress.
//
// Read from the shared list rather than retyped: the hand-written copy that used
// to live here covered 5 of the 9 entries, so `content.js`, `capture-rows.js`,
// `answer-replace.js` and `probe-fields.js` were shipped unguarded.
const INJECTED_CLASSIC_SCRIPTS = INJECTED_SCRIPT_FILES;

// HOW the check works, and why it is not a text scan.
//
// This used to strip comments and string literals with regexes and then look for
// the token `import`/`export` in what was left. That is unsound, and measurably
// so: a single apostrophe in a comment or an unbalanced quote inside a string
// table desynchronises the single-quote stripper, which then swallows everything
// up to the next apostrophe — including any `import` after it. Appending
// `import{x}from"./y.js"` to the real built `capture.js`, `fill.js`,
// `probe-fields.js`, `capture-rows.js` or `answer-replace.js` was NOT detected;
// only the small `content.js` was. The guard read as if it covered these files
// and did not.
//
// So ask the JavaScript parser instead of pattern-matching around it.
// `new Script(src)` compiles in the SCRIPT goal — exactly the goal
// `chrome.scripting.executeScript({ files: [...] })` evaluates these in — and
// throws `SyntaxError: Cannot use import statement outside a module` on any ES
// module syntax. That is not a heuristic standing in for the invariant; it IS
// the invariant. Compile only, never run: undefined globals like `document` and
// `chrome` are irrelevant because nothing executes.
//
// Script-goal compilation cannot catch a DYNAMIC `import(...)` — it is valid
// syntax in a script — so that one case still needs a pattern, matched against
// the raw source (0 false positives across all 18 built artifacts).
const DYNAMIC_IMPORT_RE = /\bimport\s*\(/;

function failsToCompileAsClassicScript(src) {
  try {
    new Script(src);
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  if (DYNAMIC_IMPORT_RE.test(src)) {
    return 'contains a dynamic import(), which resolves nothing in an injected classic script';
  }
  return null;
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
    const reason = failsToCompileAsClassicScript(readFileSync(filePath, 'utf8'));
    if (reason) {
      console.error(
        `error: ${rel(filePath)} does not parse as a classic script (${reason}) — ` +
          `chrome.scripting.executeScript({ files: [...] }) can't load ES modules. ` +
          `The injectedEntries isolated-build guarantee in vite.config.mts has regressed.`
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
