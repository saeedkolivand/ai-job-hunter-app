import type { ReactNode } from 'react';

// Off-site link (GitHub) — always new tab, always noopener/noreferrer.
export function ExternalLink({ href, children }: { href: string; children: ReactNode }) {
  return (
    <a href={href} target="_blank" rel="noopener noreferrer">
      {children}
    </a>
  );
}
