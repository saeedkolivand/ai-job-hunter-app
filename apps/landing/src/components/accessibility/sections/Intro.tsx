import { BackLink } from '@/components/BackLink';

import { LAST_UPDATED } from './Footer';

// Intro + "Commitment", "Scope" and "Standard" blocks of /accessibility, split
// out of AccessibilityBody.tsx purely for file size — same shape as
// components/privacy/sections/IntroShort.tsx. No props.
export function Intro() {
  return (
    <>
      <BackLink />

      <h1>Accessibility Statement</h1>
      <p className="updated">Last updated: {LAST_UPDATED}</p>

      <p className="lede">
        This is an accessibility statement for AI Job Hunter: the landing site at{' '}
        <code>aijobhunter.app</code>, the desktop app, and the browser extension. It says plainly
        what standard we're aiming for, how close we actually are today, what's known to be broken,
        how that was checked, and how to tell us about something we missed.
      </p>

      {/* Commitment */}
      <h2 id="commitment">
        Commitment{' '}
        <a className="anchor" href="#commitment" aria-label="Link to Commitment">
          #
        </a>
      </h2>
      <p>
        Publishing this statement isn't legally required for this product — the EU Accessibility Act
        has a microenterprise exemption (fewer than 10 staff, under €2M turnover) that covers a solo
        maintainer's free app, and no US statute requires one from a private site. It's here anyway
        because this is a job-hunting tool, its users include disabled job seekers, and a statement
        next to <a href="/privacy">/privacy</a> matches how this site already treats disclosure:
        honestly, or not at all.{' '}
        <b>This is a self-assessment, not a compliance claim or a certification.</b>
      </p>

      {/* Scope */}
      <h2 id="scope">
        Scope{' '}
        <a className="anchor" href="#scope" aria-label="Link to Scope">
          #
        </a>
      </h2>
      <ul>
        <li>
          <b>Landing site</b> — every public page at <code>aijobhunter.app</code>, including this
          one.
        </li>
        <li>
          <b>Desktop app</b> — AI Job Hunter, the Tauri app for macOS, Windows and Linux (see{' '}
          <a href="/download">/download</a>).
        </li>
        <li>
          <b>Browser extension</b> — AI Job Hunter — Job Importer for Chrome and Firefox, which
          injects a small import button and an optional autofill panel into job-board pages (see{' '}
          <a href="/privacy">/privacy</a> for what it does and doesn't send).
        </li>
      </ul>

      {/* Standard */}
      <h2 id="standard">
        Standard{' '}
        <a className="anchor" href="#standard" aria-label="Link to Standard">
          #
        </a>
      </h2>
      <p>
        The target across all three surfaces is <b>WCAG 2.2, level AA</b>.
      </p>
    </>
  );
}
