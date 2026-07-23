import { CHROME_EXT, FIREFOX_EXT, GITHUB_REPO, SPONSOR } from '@/lib/site-links';

// The marketing-tier footer (`<footer><p class="byline">…<p class="foot-links">…`)
// shared byte-for-byte by how-it-works/privacy/download body.html. `current`
// renders that one nav item as plain text instead of a link, matching how each
// page today omits the link to itself. Separators are explicit `{' · '}`
// strings — JSX drops whitespace-only text between elements, so relying on
// newlines here would silently lose the ` · ` that scripts/check-parity.mjs
// checks for.
export function SiteFooter({ current }: { current?: 'download' | 'privacy' }) {
  return (
    <footer>
      <p className="byline">made by Saeed, between rejections.</p>
      <p className="foot-links">
        <a href="/">home</a>
        {' · '}
        {current === 'download' ? 'download' : <a href="/download">download</a>}
        {' · '}
        {current === 'privacy' ? 'privacy' : <a href="/privacy">privacy</a>}
        {' · '}
        <a href="/creature">▶ the short film</a>
        {' · '}
        <a href="/storybook/">design system</a>
        {' · '}
        <a href={GITHUB_REPO} target="_blank" rel="noopener noreferrer">
          GitHub
        </a>
        {' · '}
        <a href={CHROME_EXT} target="_blank" rel="noopener noreferrer">
          Chrome extension
        </a>
        {' · '}
        <a href={FIREFOX_EXT} target="_blank" rel="noopener noreferrer">
          Firefox extension
        </a>
        {' · '}
        <a href={SPONSOR} target="_blank" rel="noopener noreferrer">
          ♥ sponsor
        </a>
      </p>
    </footer>
  );
}
