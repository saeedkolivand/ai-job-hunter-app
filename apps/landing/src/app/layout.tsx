import type { Metadata } from "next";

import GLLoader from "./GLLoader";

import "./globals.css";

// Inline SVG data-URI favicon, transcribed verbatim from landing/index.html.
const FAVICON =
  "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'><circle cx='16' cy='16' r='12' fill='none' stroke='black' stroke-width='2'/><path d='M11 14 q2 2 4 0 M17 14 q2 2 4 0 M11 22 q5 -4 10 0' fill='none' stroke='black' stroke-width='2' stroke-linecap='round'/><ellipse cx='10' cy='18' rx='1.6' ry='2.8' fill='deepskyblue'/></svg>";

const OG_IMAGE = "https://aijobhunter.app/og-card.jpg";
const OG_DESC =
  "I sent 1,000 applications and got 0 replies. So I built a robot. It does everything but press send.";

export const metadata: Metadata = {
  metadataBase: new URL("https://aijobhunter.app"),
  title: "AI Job Hunter \u2014 please hire him",
  description:
    "Covers 24 job boards (direct scrapers + Adzuna/JSearch aggregator), writes your cover letters, does everything but hit submit. A real desktop app. Also a cry for help.",
  alternates: {
    canonical: "https://aijobhunter.app/",
  },
  verification: {
    google: "kP-_YvYx7Q5rIN5F8DIwbG3-oKVXMjr9BlNa1holc0M",
  },
  icons: {
    icon: FAVICON,
  },
  openGraph: {
    title: "IT DOES EVERYTHING ELSE.",
    description: OG_DESC,
    type: "website",
    url: "https://aijobhunter.app/",
    images: [
      {
        url: OG_IMAGE,
        width: 1200,
        height: 630,
        alt: "AI Job Hunter \u2014 IT DOES EVERYTHING ELSE. Covers 24 job boards, writes your cover letters, does everything but hit submit.",
      },
    ],
  },
  twitter: {
    card: "summary_large_image",
    title: "IT DOES EVERYTHING ELSE.",
    description: OG_DESC,
    images: [OG_IMAGE],
  },
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <head>
        {/* React 19 hoists these font links into <head>; kept verbatim from landing/index.html. */}
        <link rel="preconnect" href="https://fonts.googleapis.com" />
        <link rel="preconnect" href="https://fonts.gstatic.com" crossOrigin="anonymous" />
        <link
          href="https://fonts.googleapis.com/css2?family=Anton&family=Caveat:wght@600;700&family=Gloria+Hallelujah&family=Patrick+Hand&family=Space+Mono:wght@400;700&display=swap"
          rel="stylesheet"
        />
      </head>
      <body>
        {children}
        {/* Capability-gated GL takeover. Mounts the WebGL Experience only when
            the gate passes; otherwise renders nothing and legacy boots. */}
        <GLLoader />
      </body>
    </html>
  );
}
