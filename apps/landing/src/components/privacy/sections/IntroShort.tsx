import { BackLink } from '@/components/BackLink';

// Intro + "The short version" block of /privacy, split out of
// PrivacyBody.tsx purely for file size — verbatim ported markup, no props.
// See PrivacyBody.tsx for the shared conversion notes. Nothing scripts this
// page (privacy-0.js is a console easter egg), but scripts/check-parity.mjs
// pins its copy and anchor hrefs (ADR 0018).
export function IntroShort() {
  return (
    <>
      <BackLink />

      <h1>Privacy Policy</h1>
      <p className="updated">Last updated: 30 June 2026</p>

      <p className="lede">
        AI Job Hunter is a <b>local-first desktop app</b> with an optional{' '}
        <b>companion browser extension</b>. There are no accounts to sign up for, and most of what
        the app does never leaves your computer. The desktop app does send <b>crash reports</b> so
        we can fix what breaks — on by default, switchable off, and never containing your résumés or
        job data. The browser extension sends nothing at all. The parts that <i>do</i> leave your
        computer are spelled out plainly below, because store reviewers read this against the actual
        code, and so should you.
      </p>

      {/* Short version */}
      <h2 id="short">
        The short version{' '}
        <a className="anchor" href="#short" aria-label="Link to The short version">
          #
        </a>
      </h2>
      <div className="card tldr">
        <ul>
          <li>
            <b>No accounts.</b> Nothing to register. We never see your data because there is no "we"
            server to see it.
          </li>
          <li>
            <b>No behavioural analytics or ad tracking.</b> No Google Analytics, no PostHog, no
            Segment, no Mixpanel, no advertising or cross-site tracking of any kind. Nothing follows
            you around, and nothing records what you do in the app.
          </li>
          <li>
            <b>Crash reports, on by default, from the desktop app only.</b> When something breaks,
            the error and its stack trace, your operating system and the app version are sent to{' '}
            <b>Sentry</b> so it can be fixed. Paths, links, e-mail addresses and credentials are
            stripped before anything is sent. You are asked during setup and can switch it off there
            or in <i>Settings → Privacy</i> at any time. The browser extension is excluded entirely.
          </li>
          <li>
            <b>The browser extension is loopback-only.</b> It talks to <i>your own</i> running
            desktop app on <code>127.0.0.1</code> and nowhere else.
          </li>
          <li>
            <b>
              The desktop app talks to a third-party AI provider only when you set one up and run an
              AI feature
            </b>{' '}
            — using <i>your own</i> API key (or your own local model). That content is governed by
            that provider's privacy policy.
          </li>
          <li>
            <b>Your data lives on your machine</b> — résumés, job history and settings are stored
            locally in your OS application-data directory. Secrets (API keys, board passwords) go in
            your operating system's keychain, not plain files.
          </li>
        </ul>
      </div>
    </>
  );
}
