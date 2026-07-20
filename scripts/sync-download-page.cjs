#!/usr/bin/env node
/*
 * Sync the landing download page (apps/landing/download.html) to a published
 * release's installer assets: regenerates the per-platform download buttons
 * (macOS / Windows / Linux) between the <!-- downloads:start --> /
 * <!-- downloads:end --> markers, pinned to the given version.
 *
 *   node scripts/sync-download-page.cjs <version>     # e.g. 0.103.0
 *
 * Run by the release pipeline's `update-download-page` job AFTER the installer
 * build (mirrors `sync-cask.cjs`) — that's the only point where the versioned
 * assets the buttons link to actually exist on the GitHub Release. The push of
 * the rewritten page to main re-triggers the Pages deploy (pages.yml watches
 * apps/landing/**), so the live site updates without `[skip ci]`.
 *
 * The asset filenames mirror the release notes Downloads table in
 * .github/workflows/release.yml — keep the two in sync if either changes.
 */
const fs = require('node:fs');
const path = require('node:path');

const REPO = 'https://github.com/saeedkolivand/ai-job-hunter-app';

const [version] = process.argv.slice(2);
if (!version) {
  console.error('usage: sync-download-page.cjs <version>');
  process.exit(1);
}

// Mirrors the version validation in release.yml's "Resolve Version" step.
const VERSION_RE = /^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$/;
if (!VERSION_RE.test(version)) {
  console.error(`invalid version (expected e.g. 0.103.0): ${version}`);
  process.exit(1);
}

/** The auto-generated platform-cards block, pinned to `v`. */
function buildBlock(v) {
  const base = `${REPO}/releases/download/v${v}`;
  return [
    '<!-- downloads:start (auto-synced on release build by scripts/sync-download-page.cjs — do not edit by hand) -->',
    `    <p class="dl-version">latest installer build: <b>v${v}</b></p>`,
    '',
    '    <div class="pcard">',
    '      <div class="pc-head"><div class="pc-ico">🍎</div><h2>macOS</h2></div>',
    '      <div class="pc-actions">',
    `        <a class="dl-btn" href="${base}/macos-AI-Job-Hunter_${v}_aarch64-apple-silicon.dmg">Apple Silicon · .dmg</a>`,
    `        <a class="dl-btn alt" href="${base}/macos-AI-Job-Hunter_${v}_x64-intel.dmg">Intel · .dmg</a>`,
    '      </div>',
    '      <p class="dl-note">macOS says it\'s "damaged"? It isn\'t — just unsigned. Clear the quarantine flag once:<br><code class="copy-cmd" role="button" tabindex="0" title="click to copy" data-copy=\'xattr -cr "/Applications/AI Job Hunter.app"\'>xattr -cr "/Applications/AI Job Hunter.app"</code><br>Or install it with Homebrew — tap the repo once, then install:<br><code class="copy-cmd" role="button" tabindex="0" title="click to copy" data-copy="brew tap saeedkolivand/ai-job-hunter-app https://github.com/saeedkolivand/ai-job-hunter-app">brew tap saeedkolivand/ai-job-hunter-app https://github.com/saeedkolivand/ai-job-hunter-app</code><br><code class="copy-cmd" role="button" tabindex="0" title="click to copy" data-copy="brew install --cask ai-job-hunter">brew install --cask ai-job-hunter</code>.</p>',
    '    </div>',
    '',
    '    <div class="pcard">',
    '      <div class="pc-head"><div class="pc-ico">🪟</div><h2>Windows</h2></div>',
    '      <div class="pc-actions">',
    `        <a class="dl-btn" href="${base}/windows-AI-Job-Hunter_${v}_x64-setup.exe">Installer · .exe</a>`,
    `        <a class="dl-btn alt" href="${base}/windows-AI-Job-Hunter_${v}_x64_en-US.msi">.msi</a>`,
    '      </div>',
    '      <p class="dl-note">SmartScreen may warn (unsigned). Click "More info" → "Run anyway".</p>',
    '    </div>',
    '',
    '    <div class="pcard">',
    '      <div class="pc-head"><div class="pc-ico">🐧</div><h2>Linux</h2></div>',
    '      <div class="pc-actions">',
    `        <a class="dl-btn" href="${base}/linux-AI-Job-Hunter_${v}_amd64.AppImage">.AppImage</a>`,
    `        <a class="dl-btn alt" href="${base}/linux-AI-Job-Hunter_${v}_amd64.deb">.deb</a>`,
    `        <a class="dl-btn alt" href="${base}/linux-AI-Job-Hunter-${v}-1.x86_64.rpm">.rpm</a>`,
    '      </div>',
    '      <p class="dl-note"><code>chmod +x</code> the AppImage, then run it.</p>',
    '    </div>',
    '<!-- downloads:end -->',
  ].join('\n');
}

const pagePath = path.join(__dirname, '..', 'apps', 'landing', 'download.html');
const before = fs.readFileSync(pagePath, 'utf8');

const BLOCK_RE = /<!-- downloads:start[\s\S]*?downloads:end -->/;
if (!BLOCK_RE.test(before)) {
  console.error('download.html format unexpected — downloads:start/end markers not found');
  process.exit(1);
}

const after = before.replace(BLOCK_RE, buildBlock(version));

if (after === before) {
  console.log(`Download page already pinned to v${version} — no change.`);
  process.exit(0);
}

fs.writeFileSync(pagePath, after);
console.log(`Download page synced to v${version} (macOS / Windows / Linux buttons pinned).`);
