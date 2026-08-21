import type { Installers } from '@/lib/version';

// Reproduces the old buildDownloadsHtml() markup (the release-pipeline-stamped
// downloads:start/end block) as JSX, wrapped in the `#downloads-block` node the
// old page.tsx string-injected — DownloadFreshness ('use client', untouched)
// finds it by id at runtime and mutates it in place.
//
// CRITICAL INVARIANT: the seven `.dl-btn` anchors below MUST stay in this exact
// positional order — macArm, macIntel, winExe, winMsi, linuxAppImage, linuxDeb,
// linuxRpm — because DownloadFreshness swaps hrefs by `querySelectorAll` index,
// not by any per-anchor identifier. Never reorder/insert/remove a `.dl-btn`
// anchor here without updating DownloadFreshness.tsx (and its test) in lockstep.
//
// Each anchor also carries `data-platform`, which is what DownloadCounts keys
// its download pills off — an explicit identifier rather than the positional
// index above. DownloadFreshness could be moved onto it too; that swap is not
// this change's to make, so the positional contract stands as written.
export function DownloadCards({
  version,
  installers,
}: {
  version: string;
  installers: Installers;
}) {
  return (
    <div id="downloads-block" style={{ display: 'contents' }}>
      <p className="dl-version">
        latest installer build: <b>v{version}</b>
      </p>

      <div className="pcard">
        <div className="pc-head">
          <div className="pc-ico">🍎</div>
          <h2>macOS</h2>
        </div>
        <div className="pc-actions">
          <a className="dl-btn" data-platform="macArm" href={installers.macArm}>
            Apple Silicon · .dmg
          </a>
          <a className="dl-btn alt" data-platform="macIntel" href={installers.macIntel}>
            Intel · .dmg
          </a>
        </div>
        <p className="dl-note">
          macOS says it&apos;s &quot;damaged&quot;? It isn&apos;t — just unsigned. Open the{' '}
          <b>Terminal</b> app (press ⌘-Space, type &quot;Terminal&quot;, hit Enter) and run this
          once to clear the quarantine flag:
          <br />
          <code
            className="copy-cmd"
            role="button"
            tabIndex={0}
            title="click to copy"
            data-copy='xattr -cr "/Applications/AI Job Hunter.app"'
          >
            xattr -cr &quot;/Applications/AI Job Hunter.app&quot;
          </code>
          <br />
          Or install it with Homebrew — tap the repo once, then install:
          <br />
          <code
            className="copy-cmd"
            role="button"
            tabIndex={0}
            title="click to copy"
            data-copy="brew tap saeedkolivand/ai-job-hunter-app https://github.com/saeedkolivand/ai-job-hunter-app"
          >
            brew tap saeedkolivand/ai-job-hunter-app
            https://github.com/saeedkolivand/ai-job-hunter-app
          </code>
          <br />
          <code
            className="copy-cmd"
            role="button"
            tabIndex={0}
            title="click to copy"
            data-copy="brew install --cask ai-job-hunter"
          >
            brew install --cask ai-job-hunter
          </code>
          .
        </p>
      </div>

      <div className="pcard">
        <div className="pc-head">
          <div className="pc-ico">🪟</div>
          <h2>Windows</h2>
        </div>
        <div className="pc-actions">
          <a className="dl-btn" data-platform="winExe" href={installers.winExe}>
            Installer · .exe
          </a>
          <a className="dl-btn alt" data-platform="winMsi" href={installers.winMsi}>
            .msi
          </a>
        </div>
        <p className="dl-note">
          SmartScreen may warn (unsigned). Click &quot;More info&quot; → &quot;Run anyway&quot;.
        </p>
      </div>

      <div className="pcard">
        <div className="pc-head">
          <div className="pc-ico">🐧</div>
          <h2>Linux</h2>
        </div>
        <div className="pc-actions">
          <a className="dl-btn" data-platform="linuxAppImage" href={installers.linuxAppImage}>
            .AppImage
          </a>
          <a className="dl-btn alt" data-platform="linuxDeb" href={installers.linuxDeb}>
            .deb
          </a>
          <a className="dl-btn alt" data-platform="linuxRpm" href={installers.linuxRpm}>
            .rpm
          </a>
        </div>
        <p className="dl-note">
          <code>chmod +x</code> the AppImage, then run it.
        </p>
      </div>
    </div>
  );
}
