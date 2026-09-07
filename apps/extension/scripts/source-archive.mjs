// Firefox AMO source-code archive builder for the AI Job Hunter extension.
//
// AMO reviewers REBUILD a bundled add-on from the submitted source archive and
// diff the result against the uploaded package; a mismatch (or a missing file,
// or missing build instructions) delays the review or gets the add-on pulled.
// So the archive has to be the complete source — every workspace package, the
// lockfile, and a README at the archive ROOT naming the OS, the exact tool
// versions and the exact commands.
//
// `git archive` of the release commit IS that archive: it is provably the whole
// tracked tree (no ignore list to keep honest, nothing to forget), and it needs
// no `zip` binary on any platform. The one file that is NOT in the tree — the
// build README — is injected by `git archive --add-file` (git >= 2.38), which
// places it at the archive root.
//
// The README's tool versions are READ FROM THE TOOLCHAIN THAT RUNS THIS SCRIPT
// (`process.versions.node`, the root `packageManager` pin) rather than typed in,
// so the instructions a reviewer follows can never drift from the environment
// that actually produced the artifact. In CI the release workflow rebuilds from
// this very archive and diffs the result against the shipped zip before
// submitting, so the versions printed here are the ones proven to reproduce.
//
// Run: pnpm -F @ajh/extension package:source
//      node apps/extension/scripts/source-archive.mjs --ref v1.2.3 --out out.zip

import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir, type as osType, release as osRelease } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { INJECTED_SCRIPT_FILES } from '../injected-entries.mjs';

const __filename = fileURLToPath(import.meta.url);
const EXT_ROOT = path.resolve(path.dirname(__filename), '..');
const REPO_ROOT = path.resolve(EXT_ROOT, '..', '..');
const DIST = path.join(EXT_ROOT, 'dist');

const require = createRequire(import.meta.url);
const rel = (p) => path.relative(REPO_ROOT, p).split(path.sep).join('/');

/** AMO refuses a source archive larger than this. */
const MAX_ARCHIVE_BYTES = 200 * 1024 * 1024;

/** Filename `--add-file` gives the injected README inside the archive. */
export const README_ENTRY_NAME = 'SOURCE_BUILD_README.md';

/** Store filename for the source archive of a given add-on version. */
export function sourceArchiveName(version) {
  return `ai-job-hunter-extension-source-${version}.zip`;
}

/**
 * Best-effort human label for the OS that produced the archive. AMO asks which
 * operating system the build was run on; on the Linux runners `/etc/os-release`
 * gives the distro + release ("Ubuntu 24.04.2 LTS"), which is the useful answer.
 */
export function detectOs(readOsRelease = () => readFileSync('/etc/os-release', 'utf8')) {
  try {
    const match = /^PRETTY_NAME="?([^"\n]+)"?/m.exec(readOsRelease());
    if (match?.[1]) return match[1];
  } catch {
    // Not a Linux distro (or unreadable) — fall through to the generic label.
  }
  return `${osType()} ${osRelease()}`;
}

/**
 * The Node range the build actually supports, read from the INSTALLED Vite's
 * `engines.node`. Not hand-written: "any 22.x" was wrong (the locked Vite wants
 * `^20.19.0 || >=22.12.0`), and a reviewer following a wrong range gets a build
 * failure we told them to expect success from. `null` if Vite cannot be
 * resolved, which the README turns into a pointer instead of a claim.
 */
export function viteNodeRange(resolveVitePackage = () => require('vite/package.json')) {
  try {
    const range = resolveVitePackage()?.engines?.node;
    return typeof range === 'string' && range.trim() ? range : null;
  } catch {
    return null;
  }
}

/**
 * The build README placed at the archive root. Every version in it is passed in
 * from the live toolchain, the unminified-script list comes from the build's own
 * {@link INJECTED_SCRIPT_FILES}, and the Node range comes from Vite's own
 * `engines` — nothing here is a literal that can go stale.
 */
export function buildReadme({
  version,
  os,
  node,
  packageManager,
  injectedScripts = INJECTED_SCRIPT_FILES,
  nodeRange = viteNodeRange(),
}) {
  const nodeRequirement = nodeRange
    ? `Vite pins the supported range at \`${nodeRange}\``
    : "the supported range is Vite's own `engines.node` (see `node_modules/vite/package.json` after the install below)";
  return `# AI Job Hunter — Firefox add-on source build (v${version})

This archive is the complete, unmodified source of the \`ai-job-hunter\`
monorepo at the commit that produced the submitted add-on package. The add-on
lives in \`apps/extension/\`; it is bundled with Vite, which is why AMO needs
this archive.

## Build environment

| Tool | Version this release was built and verified with |
| ---- | ------------------------------------------------ |
| OS   | ${os} |
| Node | ${node} |
| pnpm | ${packageManager.replace(/^pnpm@/, '')} |

- **Node.js** — <https://nodejs.org/en/download>. ${nodeRequirement}; the exact
  version above is the one that built and verified the submitted package.
- **pnpm** — <https://pnpm.io/installation>. The version is pinned by the
  \`packageManager\` field of the root \`package.json\`, so the simplest install
  is Corepack, which ships with Node:

  \`\`\`
  corepack enable
  corepack prepare ${packageManager} --activate
  \`\`\`

  \`npm install -g ${packageManager}\` works too.

## Build steps

Run these three commands from the **root of this archive**:

\`\`\`
pnpm install --frozen-lockfile
pnpm -F @ajh/shared build
pnpm -F @ajh/extension build:firefox
\`\`\`

The result is \`apps/extension/dist/firefox/\` — the exact directory that was
zipped into the submitted add-on package (\`manifest.json\` at the zip root).

## Notes for the reviewer

- Every dependency is fetched from the public npm registry and pinned to an
  exact version + integrity hash by \`pnpm-lock.yaml\`; \`--frozen-lockfile\`
  fails rather than resolving anything new. The build itself needs no network.
- \`manifest.json\` is generated at build time from \`apps/extension/src/manifest.ts\`.
  There is no remotely-hosted code, no \`eval\`, and no runtime code fetch.
- These ${injectedScripts.length} files are emitted **unminified on purpose**:
  ${injectedScripts.map((f) => `\`${f}\``).join(', ')}.
  Each is injected with \`chrome.scripting.executeScript({ files: [...] })\` as a
  classic script, and several return their result to the background page as a
  completion value — something a minifier is entitled to fold away. They are also
  each built in their own isolated Rollup pass so no shared chunk is hoisted out
  and \`import\`ed, which classic-script injection cannot load. The reasoning is
  in \`apps/extension/vite.config.mts\`; the list itself is
  \`apps/extension/injected-entries.mjs\`, which that config and this README are
  both generated from.
- \`apps/desktop/\` (a Tauri app) and \`apps/landing/\` are in this archive because
  they share the monorepo and its lockfile. Neither is needed to build the add-on
  and neither ships inside it; \`apps/extension/README.md\` describes what the
  add-on does and how it talks to the desktop app over loopback.
`;
}

/**
 * `git archive` reads the tree from `ref`, but EVERYTHING else in the archive —
 * the version, the injected-script list, the README's tool versions — is read
 * from the checked-out worktree. Point `--ref` at a different commit and you get
 * an archive whose contents and whose build README disagree, and AMO's whole
 * reason for wanting the archive (rebuild it, diff it against the package) no
 * longer holds, because the reproducibility gate only ever ran against the tree
 * that was built. So refuse rather than emit a mislabelled archive.
 *
 * Returns `null` when `ref` resolves to the same commit as `HEAD`, otherwise the
 * message explaining which two commits disagreed.
 */
export function refMismatch(ref, resolve) {
  if (ref === 'HEAD') return null;
  const head = resolve('HEAD');
  const target = resolve(`${ref}^{commit}`);
  if (!target) return `"${ref}" is not a commit in this repository`;
  if (!head) return 'HEAD does not resolve to a commit';
  if (head === target) return null;
  return (
    `--ref ${ref} is ${target}, but the checked-out worktree is ${head}. ` +
    `The archive's tree would come from ${ref} while its build README, version ` +
    `and file list come from the worktree, so it would be mislabelled. ` +
    `Check out ${ref}, rebuild, and run this again.`
  );
}

function resolveCommit(ref) {
  const res = spawnSync('git', ['rev-parse', '--verify', '--quiet', ref], {
    cwd: REPO_ROOT,
    encoding: 'utf8',
  });
  return res.status === 0 ? res.stdout.trim() : null;
}

/** `--ref <tree-ish>` / `--out <path>`; both optional. */
export function parseArgs(argv) {
  const args = { ref: 'HEAD', out: undefined };
  for (let i = 0; i < argv.length; i += 1) {
    if (argv[i] === '--ref' && argv[i + 1]) args.ref = argv[(i += 1)];
    else if (argv[i] === '--out' && argv[i + 1]) args.out = argv[(i += 1)];
    else {
      console.error(
        `error: unexpected argument "${argv[i]}" (usage: --ref <tree-ish> --out <zip>)`
      );
      process.exit(1);
    }
  }
  return args;
}

function main() {
  const { ref, out: outArg } = parseArgs(process.argv.slice(2));

  const mismatch = refMismatch(ref, resolveCommit);
  if (mismatch) {
    console.error(`error: ${mismatch}`);
    process.exit(1);
  }

  const { version } = require('../package.json');
  const { packageManager } = require('../../../package.json');
  const out = outArg ? path.resolve(outArg) : path.join(DIST, sourceArchiveName(version));

  const staging = mkdtempSync(path.join(tmpdir(), 'ajh-ext-source-'));
  const readmePath = path.join(staging, README_ENTRY_NAME);
  writeFileSync(
    readmePath,
    buildReadme({ version, os: detectOs(), node: process.versions.node, packageManager })
  );

  mkdirSync(path.dirname(out), { recursive: true });
  rmSync(out, { force: true });
  // `--add-file` puts the README at the archive root under its basename; it
  // needs git >= 2.38 (older git fails loudly with "unknown option").
  const res = spawnSync(
    'git',
    ['archive', '--format=zip', `--add-file=${readmePath}`, '-o', out, ref],
    { cwd: REPO_ROOT, stdio: 'inherit' }
  );
  rmSync(staging, { recursive: true, force: true });

  if (res.error) {
    console.error(`error: could not run git (${res.error.message})`);
    process.exit(1);
  }
  if (res.status !== 0) {
    console.error(`error: git archive of "${ref}" failed (exit ${res.status})`);
    process.exit(res.status ?? 1);
  }

  const bytes = statSync(out).size;
  if (bytes > MAX_ARCHIVE_BYTES) {
    console.error(
      `error: ${rel(out)} is ${(bytes / 1024 / 1024).toFixed(1)} MB — AMO rejects source ` +
        `archives over ${MAX_ARCHIVE_BYTES / 1024 / 1024} MB.`
    );
    process.exit(1);
  }
  console.log(`packaged ${rel(out)}  ${bytes} bytes (${(bytes / 1024 / 1024).toFixed(1)} MB)`);
}

// Importable for tests; only builds an archive when run as a script.
if (process.argv[1] && path.resolve(process.argv[1]) === __filename) main();
