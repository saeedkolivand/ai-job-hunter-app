import { BackLink } from '@/components/BackLink';
import { SiteFooter } from '@/components/SiteFooter';
import { CHROME_EXT, FIREFOX_EXT } from '@/lib/site-links';
import type { Installers } from '@/lib/version';

import { DownloadCards } from './DownloadCards';

// The /download route body, ported 1:1 from src/content/download/body.html.
// Root is a literal `display:contents` div — the same wrapper the old
// `<RawHtml>` injected — so the serialized DOM stays identical to the baseline.
export function DownloadBody({ version, installers }: { version: string; installers: Installers }) {
  return (
    <div style={{ display: 'contents' }}>
      <main className="wrap">
        <BackLink />

        <h1>Take the app</h1>

        <p className="lede">
          A <b>local-first desktop app</b> — it scrapes the job boards, ranks the matches, and
          writes the whole application on your own machine.{' '}
          <b>Free for personal use. Source-available.</b> No accounts, no telemetry, no upsell. It
          is also <b>unsigned</b>, because code-signing certificates cost money and I, famously, do
          not have a job. Your operating system will fret. The notes under each button tell you how
          to reassure it.
        </p>

        <div className="platforms">
          <DownloadCards version={version} installers={installers} />
        </div>

        <p className="auto-note">
          Installed apps <b>auto-update</b> — grab it once and you never need this page again.
          Builds are signed for the updater, just not for Gatekeeper/SmartScreen. macOS users can
          also install with Homebrew —<br />
          <code
            className="copy-cmd"
            role="button"
            tabIndex={0}
            aria-label="Click to copy: brew tap saeedkolivand/ai-job-hunter-app https://github.com/saeedkolivand/ai-job-hunter-app"
            data-copy="brew tap saeedkolivand/ai-job-hunter-app https://github.com/saeedkolivand/ai-job-hunter-app"
          >
            brew tap saeedkolivand/ai-job-hunter-app
            https://github.com/saeedkolivand/ai-job-hunter-app
          </code>
          <br />
          then{' '}
          <code
            className="copy-cmd"
            role="button"
            tabIndex={0}
            aria-label="Click to copy: brew install --cask ai-job-hunter"
            data-copy="brew install --cask ai-job-hunter"
          >
            brew install --cask ai-job-hunter
          </code>
          .
        </p>

        <hr className="scrawl" />

        <h2>The browser extension</h2>
        <p className="ext-intro" style={{ marginTop: '14px' }}>
          Optional companion. When you&apos;re staring at a job posting, it hands that page straight
          to the desktop app running on the same machine — native messaging (or loopback) — nothing
          leaves your computer. Inert unless the app is running and paired.
        </p>

        <div className="ext-grid">
          <div className="ext-card">
            <div className="pc-ico" aria-hidden="true">
              🧩
            </div>
            <h2>Chrome</h2>
            <p className="sub">also Edge, Brave, and other Chromium browsers</p>
            <a className="ext-btn" href={CHROME_EXT} target="_blank" rel="noopener noreferrer">
              Chrome Web Store →
            </a>
          </div>
          <div className="ext-card">
            <div className="pc-ico" aria-hidden="true">
              🦊
            </div>
            <h2>Firefox</h2>
            <p className="sub">on Mozilla Add-ons (AMO)</p>
            <a className="ext-btn" href={FIREFOX_EXT} target="_blank" rel="noopener noreferrer">
              Add to Firefox →
            </a>
          </div>
        </div>

        <hr className="scrawl" />

        <SiteFooter current="download" />
      </main>
    </div>
  );
}
