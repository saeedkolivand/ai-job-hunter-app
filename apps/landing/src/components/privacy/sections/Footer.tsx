import { SiteFooter } from '@/components/SiteFooter';

// "Your control" / "Changes" / "Contact" blocks + the site footer of
// /privacy, split out of PrivacyBody.tsx purely for file size — verbatim
// ported markup, no props. See PrivacyBody.tsx for the shared conversion
// notes. Nothing scripts this page (privacy-0.js is a console easter egg),
// but scripts/check-parity.mjs pins its copy and anchor hrefs (ADR 0018).

// The single source of truth for the "Last updated" date rendered by
// sections/IntroShort.tsx — kept here, next to the "we'll bump the date"
// promise below, so changing the policy and changing the date are one edit
// apart instead of two files apart. Mirrors
// components/accessibility/sections/Footer.tsx.
//
// This page is the privacy-policy URL filed with the Chrome Web Store and
// Firefox AMO, so a stale date here is not cosmetic: it claims a review
// happened before changes that came after it.
export const LAST_UPDATED = '2 August 2026';

export function Footer() {
  return (
    <>
      {/* Your control */}
      <h2 id="control">
        Your control{' '}
        <a className="anchor" href="#control" aria-label="Link to Your control">
          #
        </a>
      </h2>
      <ul>
        <li>
          <b>Rotate the pairing token</b> any time from the app's{' '}
          <i>Settings → Browser extension</i>. Re-pairing invalidates the old token.
        </li>
        <li>
          <b>Uninstall the extension</b> to remove its stored pairing token — it lives only in the
          browser's extension storage.
        </li>
        <li>
          <b>Delete your local app data</b> by removing the app's data directory and clearing the
          saved secrets from your OS keychain. Your résumés, job data and generated documents exist
          only on your machine, so deleting them locally deletes them everywhere.
        </li>
        <li>
          <b>Erase crash reports</b> — the one thing that is not local. If crash reporting was on,
          reports are held by Sentry for 30 days. Switch it off in <i>Settings → Privacy</i> to stop
          new ones, and e-mail us to have existing reports deleted sooner.
        </li>
        <li>
          <b>Stay fully local</b> by choosing a local model (Ollama) for AI and embeddings — then
          nothing leaves your machine except the job-board fetches you initiate.
        </li>
      </ul>

      {/* Changes */}
      <h2 id="changes">
        Changes to this policy{' '}
        <a className="anchor" href="#changes" aria-label="Link to Changes to this policy">
          #
        </a>
      </h2>
      <p>
        If this policy changes, we'll bump the <b>"Last updated"</b> date at the top of this page
        and publish the revised version here. Material changes will be reflected in the app's store
        listings. There are no accounts, so there's no mailing list to notify — checking this page
        is the source of truth.
      </p>

      {/* Contact */}
      <h2 id="contact">
        Contact{' '}
        <a className="anchor" href="#contact" aria-label="Link to Contact">
          #
        </a>
      </h2>
      <p>
        Questions about privacy, or a data request? Email{' '}
        <a href="mailto:contact@aijobhunter.app">contact@aijobhunter.app</a>.
      </p>

      <hr className="scrawl" />

      <SiteFooter current="privacy" />
    </>
  );
}
