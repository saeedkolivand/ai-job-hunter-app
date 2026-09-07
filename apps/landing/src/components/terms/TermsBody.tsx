import { BackLink } from '@/components/BackLink';
import { SiteFooter } from '@/components/SiteFooter';
import { GITHUB_REPO } from '@/lib/site-links';

// Body markup for /terms. A new page (not a port of legacy HTML), wearing the
// same document-page chrome as /privacy and /accessibility — BackLink,
// main.wrap, .updated/.lede, <h2 id> + `#` self-link, SiteFooter — so the three
// legal pages read as one set. Kept in ONE file rather than split into
// sections/: those two pages were split purely for file size, and this page's
// copy is a fraction of theirs.
const APACHE_LICENSE = 'https://www.apache.org/licenses/LICENSE-2.0';

// The single source of truth for the "Last updated" date rendered below. The
// "Changes" section promises this date tells you when the terms last moved, so
// a copy edit and its date bump are one edit apart; TermsBody.test.tsx pins the
// rendered form. Mirrors components/privacy/sections/Footer.tsx.
export const LAST_UPDATED = '7 September 2026';

export function TermsBody() {
  return (
    <div style={{ display: 'contents' }}>
      <main className="wrap">
        <BackLink />

        <h1>Terms of Use</h1>
        <p className="updated">Last updated: {LAST_UPDATED}</p>

        <p className="lede">
          AI Job Hunter is free, open-source software published under the{' '}
          <a href={APACHE_LICENSE} target="_blank" rel="noopener noreferrer">
            Apache License 2.0
          </a>
          . These terms explain what you can expect from it and what we expect from you. They apply
          to the desktop app, the browser extension and this website.
        </p>

        {/* As is */}
        <h2 id="as-is">
          The software is provided as is{' '}
          <a className="anchor" href="#as-is" aria-label="Link to The software is provided as is">
            #
          </a>
        </h2>
        <p>
          The Apache License 2.0 governs your rights to use, copy, modify and redistribute the
          software. Sections 7 and 8 of that license apply in full: the software comes without
          warranty of any kind, and, to the fullest extent permitted by applicable law, the authors
          are not liable for damages arising from its use. Nothing here narrows the license.
        </p>

        {/* Your output */}
        <h2 id="your-output">
          You are responsible for what you send to employers{' '}
          <a
            className="anchor"
            href="#your-output"
            aria-label="Link to You are responsible for what you send to employers"
          >
            #
          </a>
        </h2>
        <p>
          The app drafts résumés, cover letters and application answers with AI models you choose.
          AI output can be wrong, outdated or invented. Read everything before you use it. The app
          never submits an application on its own; every submission is your action and your
          responsibility.
        </p>

        {/* Third parties */}
        <h2 id="third-parties">
          Third-party services stay theirs{' '}
          <a
            className="anchor"
            href="#third-parties"
            aria-label="Link to Third-party services stay theirs"
          >
            #
          </a>
        </h2>
        <p>
          The app reaches job boards, applicant-tracking systems and AI providers on your behalf,
          from your machine, with the keys and accounts you provide. Their terms, rate limits and
          acceptable-use rules apply to you, not to us. Automation features (searching, scoring,
          form filling) are for your own job search — do not use them to scrape at scale, resell
          data or act on someone else's behalf without permission.
        </p>

        {/* Your data */}
        <h2 id="your-data">
          Your data stays yours{' '}
          <a className="anchor" href="#your-data" aria-label="Link to Your data stays yours">
            #
          </a>
        </h2>
        <p>
          The software is local-first: your résumé, applications and notes are stored on your
          device, and nothing is sent anywhere you did not configure. How the website and the app
          handle data is described in the <a href="/privacy">Privacy Policy</a>.
        </p>

        {/* No account */}
        <h2 id="no-account">
          No account, no service{' '}
          <a className="anchor" href="#no-account" aria-label="Link to No account, no service">
            #
          </a>
        </h2>
        <p>
          There is no AI Job Hunter account and no hosted service behind the app. Updates are
          delivered through GitHub Releases, the Microsoft Store or your package manager; you can
          stop using the software at any time by uninstalling it.
        </p>

        {/* Changes */}
        <h2 id="changes">
          Changes to these terms{' '}
          <a className="anchor" href="#changes" aria-label="Link to Changes to these terms">
            #
          </a>
        </h2>
        <p>
          We may update these terms when the software changes. The <b>"Last updated"</b> date above
          tells you when. Continued use after a change means you accept the updated terms.
        </p>

        {/* Contact */}
        <h2 id="contact">
          Contact{' '}
          <a className="anchor" href="#contact" aria-label="Link to Contact">
            #
          </a>
        </h2>
        <p>
          Email <a href="mailto:contact@aijobhunter.app">contact@aijobhunter.app</a> · source and
          issues at{' '}
          <a href={GITHUB_REPO} target="_blank" rel="noopener noreferrer">
            github.com/saeedkolivand/ai-job-hunter-app
          </a>
          .
        </p>

        <hr className="scrawl" />

        <SiteFooter current="terms" />
      </main>
    </div>
  );
}
