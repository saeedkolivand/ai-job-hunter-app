// Drift guard for the Rust toolchain pin.
//
// The pin exists because local dev, PR CI and release builds previously each
// resolved whatever "stable" meant on the day, and silently diverged. It is
// held in two independent places that CANNOT reference each other:
//
//   * `apps/desktop/src-tauri/rust-toolchain.toml` — `[toolchain] channel`,
//     which rustup reads for local dev and for anything invoking cargo
//     directly;
//   * `dtolnay/rust-toolchain@<sha>` in `.github/` — that action does NOT read
//     the toolchain file; its pinned @rev IS the version selector.
//
// Today the only thing tying them together is a comment in three files saying
// "bump both together". This repo's own rule is that a stated invariant with
// no mechanical enforcement is a defect waiting to happen, and this one has a
// particularly quiet failure: bump the TOML alone and CI keeps building on the
// old compiler; bump the action alone and every developer's machine drifts off
// what CI validates. Neither shows up as a red build — the pin just stops
// meaning anything, which is the exact state it was created to end.
//
// What this checks:
//   1. `rust-toolchain.toml` declares an exact `channel` (a pin, not a
//      floating channel like "stable"/"beta"/"1.97").
//   2. Every `dtolnay/rust-toolchain@<sha>` use in `.github/` carries a
//      trailing `# <version>` comment — that comment is the only
//      human-readable statement of which version the SHA is, so an
//      uncommented SHA is unreviewable by construction.
//   3. Every one of those declared versions equals the TOML's channel.
//   4. `Cargo.toml`'s `rust-version` (the MSRV) is not ABOVE the pinned
//      toolchain — that combination cannot build at all.
//
// What it deliberately does NOT check: that the SHA genuinely is that version.
// That needs a network call to the action repo, and it is not the mistake
// people make — the realistic error is updating one location and forgetting
// the other, which is fully catchable offline.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = process.cwd();
const TOML = 'apps/desktop/src-tauri/rust-toolchain.toml';
const CARGO = 'apps/desktop/src-tauri/Cargo.toml';
const GH_DIR = '.github';

const errors = [];

/** Every file under `.github/`, recursively. */
function walk(dir) {
  const out = [];
  for (const entry of readdirSync(join(ROOT, dir))) {
    const rel = `${dir}/${entry}`;
    if (statSync(join(ROOT, rel)).isDirectory()) out.push(...walk(rel));
    else out.push(rel);
  }
  return out;
}

// ── 1. The pinned channel ─────────────────────────────────────────────────────
const tomlText = readFileSync(join(ROOT, TOML), 'utf8');
const channelMatch = tomlText.match(/^\s*channel\s*=\s*["']([^"']+)["']/m);

if (!channelMatch) {
  errors.push(`${TOML}: no [toolchain] channel found — the pin is the point of this file.`);
}

const channel = channelMatch?.[1];
// A pin is three dot-separated numbers. "stable", "beta", "nightly" and a
// two-part "1.97" all float, which defeats the file's stated purpose.
if (channel && !/^\d+\.\d+\.\d+$/.test(channel)) {
  errors.push(
    `${TOML}: channel "${channel}" is not an exact version. A floating channel ` +
      `(stable/beta/nightly, or a two-part "1.97") re-introduces the drift this ` +
      `file exists to prevent — pin all three components.`
  );
}

// ── 2 + 3. Every action pin agrees with it ───────────────────────────────────
const actionUses = [];
for (const file of walk(GH_DIR)) {
  if (!/\.ya?ml$/.test(file)) continue;
  const lines = readFileSync(join(ROOT, file), 'utf8').split('\n');
  lines.forEach((line, i) => {
    // Only real `uses:` lines — a bare mention inside a comment is prose.
    const m = line.match(/^\s*(?:-\s*)?uses:\s*dtolnay\/rust-toolchain@(\S+)/);
    if (!m) return;
    const version = line.match(/#\s*v?(\d+\.\d+\.\d+)\s*$/)?.[1] ?? null;
    actionUses.push({ file, line: i + 1, rev: m[1], version });
  });
}

if (actionUses.length === 0) {
  errors.push(
    `no dtolnay/rust-toolchain use found under ${GH_DIR}/ — either CI stopped ` +
      `pinning the toolchain, or this check is looking in the wrong place. ` +
      `Both are worth failing on.`
  );
}

for (const use of actionUses) {
  if (use.version === null) {
    errors.push(
      `${use.file}:${use.line}: dtolnay/rust-toolchain@${use.rev} has no trailing ` +
        `"# <version>" comment. The SHA is opaque, so that comment is the only ` +
        `way a reviewer (or this check) can tell which compiler CI runs. Add ` +
        `\`# ${channel ?? 'X.Y.Z'}\`.`
    );
    continue;
  }
  if (channel && use.version !== channel) {
    errors.push(
      `${use.file}:${use.line}: pinned to ${use.version}, but ${TOML} pins ` +
        `${channel}. Local dev and CI would build on different compilers — the ` +
        `exact divergence the pin exists to prevent. Bump both together.`
    );
  }
}

// ── 4. MSRV cannot exceed the pinned toolchain ───────────────────────────────
const rustVersion = readFileSync(join(ROOT, CARGO), 'utf8').match(
  /^\s*rust-version\s*=\s*["']([^"']+)["']/m
)?.[1];

if (rustVersion && channel) {
  const parts = (v) => v.split('.').map(Number);
  const [msrvMajor = 0, msrvMinor = 0, msrvPatch = 0] = parts(rustVersion);
  const [pinMajor, pinMinor, pinPatch] = parts(channel);
  const msrv = msrvMajor * 1e6 + msrvMinor * 1e3 + msrvPatch;
  const pin = pinMajor * 1e6 + pinMinor * 1e3 + pinPatch;
  if (msrv > pin) {
    errors.push(
      `${CARGO}: rust-version (MSRV) ${rustVersion} is NEWER than the pinned ` +
        `toolchain ${channel} — that combination cannot compile. Raise the pin ` +
        `or lower the MSRV.`
    );
  }
}

if (errors.length > 0) {
  console.error(`check:toolchain-pin FAILED — ${errors.length} issue(s):\n`);
  for (const e of errors) console.error(`  - ${e}`);
  console.error('\nFix the above, then re-run.');
  process.exit(1);
}

console.log(
  `check:toolchain-pin OK — ${TOML} pins ${channel}; ` +
    `${actionUses.length} dtolnay/rust-toolchain use(s) under ${GH_DIR}/ all declare ` +
    `the same version${rustVersion ? `; MSRV ${rustVersion} is within it` : ''}.`
);
