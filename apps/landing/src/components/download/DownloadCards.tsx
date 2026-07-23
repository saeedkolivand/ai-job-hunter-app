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
          <a className="dl-btn" href={installers.macArm}>
            Apple Silicon · .dmg
          </a>
          <a className="dl-btn alt" href={installers.macIntel}>
            Intel · .dmg
          </a>
        </div>
        <p className="dl-note">
          macOS says it&apos;s &quot;damaged&quot;? It isn&apos;t — just unsigned. Clear the
          quarantine flag once:
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
          <a className="dl-btn" href={installers.winExe}>
            Installer · .exe
          </a>
          <a className="dl-btn alt" href={installers.winMsi}>
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
          <a className="dl-btn" href={installers.linuxAppImage}>
            .AppImage
          </a>
          <a className="dl-btn alt" href={installers.linuxDeb}>
            .deb
          </a>
          <a className="dl-btn alt" href={installers.linuxRpm}>
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
