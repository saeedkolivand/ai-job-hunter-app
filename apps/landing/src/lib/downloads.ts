import type { Installers } from './version';

// The per-platform download cards, as an HTML string injected into the /download
// route between the `<div class="platforms">` markers. This mirrors the block
// the old scripts/sync-download-page.cjs stamped into download.html byte-for-byte
// (same copy, same copy-cmd chips) so the port stays pixel-identical; only the
// version + installer hrefs are data-driven (src/data/version.json). The wrapper
// carries `display:contents` so the cards remain direct flex items of
// `.platforms` (preserves the gap + nth-child rotations).
export function buildDownloadsHtml(version: string, i: Installers): string {
  return `
    <p class="dl-version">latest installer build: <b>v${version}</b></p>

    <div class="pcard">
      <div class="pc-head"><div class="pc-ico">🍎</div><h2>macOS</h2></div>
      <div class="pc-actions">
        <a class="dl-btn" href="${i.macArm}">Apple Silicon · .dmg</a>
        <a class="dl-btn alt" href="${i.macIntel}">Intel · .dmg</a>
      </div>
      <p class="dl-note">macOS says it's "damaged"? It isn't — just unsigned. Clear the quarantine flag once:<br><code class="copy-cmd" role="button" tabindex="0" title="click to copy" data-copy='xattr -cr "/Applications/AI Job Hunter.app"'>xattr -cr "/Applications/AI Job Hunter.app"</code><br>Or install it with Homebrew — tap the repo once, then install:<br><code class="copy-cmd" role="button" tabindex="0" title="click to copy" data-copy="brew tap saeedkolivand/ai-job-hunter-app https://github.com/saeedkolivand/ai-job-hunter-app">brew tap saeedkolivand/ai-job-hunter-app https://github.com/saeedkolivand/ai-job-hunter-app</code><br><code class="copy-cmd" role="button" tabindex="0" title="click to copy" data-copy="brew install --cask ai-job-hunter">brew install --cask ai-job-hunter</code>.</p>
    </div>

    <div class="pcard">
      <div class="pc-head"><div class="pc-ico">🪟</div><h2>Windows</h2></div>
      <div class="pc-actions">
        <a class="dl-btn" href="${i.winExe}">Installer · .exe</a>
        <a class="dl-btn alt" href="${i.winMsi}">.msi</a>
      </div>
      <p class="dl-note">SmartScreen may warn (unsigned). Click "More info" → "Run anyway".</p>
    </div>

    <div class="pcard">
      <div class="pc-head"><div class="pc-ico">🐧</div><h2>Linux</h2></div>
      <div class="pc-actions">
        <a class="dl-btn" href="${i.linuxAppImage}">.AppImage</a>
        <a class="dl-btn alt" href="${i.linuxDeb}">.deb</a>
        <a class="dl-btn alt" href="${i.linuxRpm}">.rpm</a>
      </div>
      <p class="dl-note"><code>chmod +x</code> the AppImage, then run it.</p>
    </div>
`;
}
