import { SiteFooter } from '@/components/SiteFooter';
import { GITHUB_REPO } from '@/lib/site-links';

// "Feedback" + "Review cadence" blocks + the site footer of /accessibility,
// split out of AccessibilityBody.tsx purely for file size — same shape as
// components/privacy/sections/Footer.tsx. No props.

// The single source of truth for the "Last updated" date rendered by
// sections/Intro.tsx — kept here, next to the review-cadence promise below,
// so bumping it is a one-constant edit. AccessibilityBody.test.tsx's
// `LAST_UPDATED` suite fails once this is more than 12 months old, per the
// promise on this page that it's reviewed at least annually.
export const LAST_UPDATED = '1 August 2026';

export function Footer() {
  return (
    <>
      {/* Feedback */}
      <h2 id="feedback">
        Feedback{' '}
        <a className="anchor" href="#feedback" aria-label="Link to Feedback">
          #
        </a>
      </h2>
      <p>
        Hit a barrier? Email <a href="mailto:contact@aijobhunter.app">contact@aijobhunter.app</a> or{' '}
        <a href={`${GITHUB_REPO}/issues`} target="_blank" rel="noopener noreferrer">
          open a GitHub issue
        </a>
        . We aim to respond within <b>5 working days</b>. If something on the site or in the app is
        genuinely blocking you, tell us what it is and we'll get you the content another way.
      </p>

      {/* Review cadence */}
      <h2 id="review">
        Review cadence{' '}
        <a className="anchor" href="#review" aria-label="Link to Review cadence">
          #
        </a>
      </h2>
      <p>
        We review this statement at least once a year, and whenever the site or app changes
        materially. If it changes, we'll bump the <b>"Last updated"</b> date at the top of this page
        and publish the revised version here.
      </p>

      <hr className="scrawl" />

      <SiteFooter current="accessibility" />
    </>
  );
}
