import type { Metadata } from 'next';
import type { ReactNode } from 'react';

// The shared shell: <html lang="en">, the common favicon, and the <body> every
// route renders into. Per-page <head> (title/description/og/twitter/canonical/
// theme-color) comes from each route's `metadata`/`viewport` export; per-page
// fonts + CSS + body + scripts are rendered inside the route (each original page
// was a self-contained document with its own globally-scoped CSS).
const FAVICON =
  "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'><circle cx='16' cy='16' r='12' fill='none' stroke='black' stroke-width='2'/><path d='M11 14 q2 2 4 0 M17 14 q2 2 4 0 M11 22 q5 -4 10 0' fill='none' stroke='black' stroke-width='2' stroke-linecap='round'/><ellipse cx='10' cy='18' rx='1.6' ry='2.8' fill='deepskyblue'/></svg>";

export const metadata: Metadata = {
  metadataBase: new URL('https://aijobhunter.app'),
  icons: { icon: FAVICON },
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
