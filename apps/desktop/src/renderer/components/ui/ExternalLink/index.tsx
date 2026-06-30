import type { AnchorHTMLAttributes, ReactNode } from 'react';

import { useOpenExternal } from '@/services';

interface ExternalLinkProps extends Omit<
  AnchorHTMLAttributes<HTMLAnchorElement>,
  'href' | 'onClick'
> {
  /** The external URL to open in the system browser. */
  href: string;
  children: ReactNode;
}

/**
 * A semantic anchor that opens an external URL in the **system browser** (never
 * the in-app webview) through the centralized opener IPC (`useOpenExternal`).
 *
 * The real `href` keeps it a proper link for accessibility (role=link, keyboard
 * activation, screen readers) while the click is intercepted so navigation
 * always leaves the webview.
 *
 * Use this for external hyperlinks. For a button/action that merely happens to
 * open a URL — especially one with side effects (analytics, tracking) — call
 * `useOpenExternal()` directly instead.
 */
export function ExternalLink({ href, children, ...rest }: ExternalLinkProps) {
  const openExternal = useOpenExternal();
  return (
    // Spread `rest` first so the routing-critical href/rel/onClick can't be
    // overridden by a caller (the props type already Omits them for safety).
    <a
      {...rest}
      href={href}
      rel="noreferrer"
      onClick={(e) => {
        e.preventDefault();
        openExternal.mutate(href);
      }}
    >
      {children}
    </a>
  );
}
