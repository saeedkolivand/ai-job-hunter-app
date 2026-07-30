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
        <b>companion browser extension</b>. We don't track you — honestly, we can barely track
        ourselves. There are no accounts to sign up for, nothing here phones home for analytics, and
        most of what the app does never leaves your computer. The parts that <i>do</i> leave your
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
            <b>No analytics, no telemetry, no tracking.</b> No Sentry, no Google Analytics, no
            PostHog — nothing.
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
