import type { ReactNode } from 'react';

import { PageStyle } from '@/components/PageStyle';
import { readStyle } from '@/lib/styles';

// The docs-tier layout. A near-black hand-drawn surface (the /agent-system look)
// with a "back to the chaos" top link, an optional eyebrow/title/lede head, and
// a mono footer. Styling is inlined per route via <PageStyle> (the exempt
// first-party-CSS injector) from the --doc-* token file + shell chrome — no
// global stylesheet, so the marketing tier keeps its own skin.
//
// Server component: it reads its CSS from disk at build time. Client interactivity
// (e.g. /mission-control) lives in `children` client components rendered inside it.
const SHELL_CSS = `${readStyle('docs-tokens.css')}\n${readStyle('doc-shell.css')}`;

export function DocShell({
  eyebrow,
  title,
  lede,
  wide = false,
  children,
}: {
  eyebrow?: string;
  title?: string;
  lede?: ReactNode;
  wide?: boolean;
  children: ReactNode;
}) {
  const hasHead = Boolean(eyebrow || title || lede);
  return (
    <>
      <PageStyle css={SHELL_CSS} />
      <div className="doc-shell">
        <header className="doc-chrome">
          <a className="doc-back" href="/">
            ← back to the chaos
          </a>
        </header>
        <main className={wide ? 'doc-shell__main is-wide' : 'doc-shell__main'}>
          {hasHead ? (
            <div className="doc-shell__head">
              {eyebrow ? <p className="doc-eyebrow">{eyebrow}</p> : null}
              {title ? <h1 className="doc-title">{title}</h1> : null}
              {lede ? <p className="doc-lede">{lede}</p> : null}
            </div>
          ) : null}
          {children}
        </main>
        <footer className="doc-shell__footer">
          <p className="doc-byline">made by Saeed, between rejections.</p>
          <p className="doc-foot-links">
            <a href="/">home</a> · <a href="/download">download</a> ·{' '}
            <a href="/agent-system">the agent fleet</a> ·{' '}
            <a href="/architecture-map">architecture</a> ·{' '}
            <a
              href="https://github.com/saeedkolivand/ai-job-hunter-app"
              target="_blank"
              rel="noopener noreferrer"
            >
              GitHub
            </a>{' '}
            ·{' '}
            <a
              href="https://github.com/sponsors/saeedkolivand"
              target="_blank"
              rel="noopener noreferrer"
            >
              ♥ sponsor
            </a>
          </p>
        </footer>
      </div>
    </>
  );
}
