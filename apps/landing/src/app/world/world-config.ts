// Config for the /world scroll-scrubbed camera-flight route. Kept as plain,
// unit-testable data (world-config.test.ts) separate from the client component
// that mounts the vendored engine (WorldClient.tsx). Asset paths are absolute
// from the site root; the media files themselves land in public/world/ later
// (rendered separately) — the paths are wired now per the approved plan.

export interface WorldSectionCta {
  primary: { label: string; href: string };
  secondary: { label: string; href: string };
}

export interface WorldSection {
  id: string;
  label: string;
  still: string;
  stillMobile: string;
  clip: string;
  clipMobile: string;
  accent: string;
  /** Viewport-heights of scroll for this scene's dive (overrides diveScroll). */
  scroll?: number;
  /** 0..1 — remaps scroll→time so the camera settles mid-scene. */
  linger?: number;
  eyebrow: string;
  title: string;
  body: string;
  tags: string[];
  /** Only the last section carries a CTA. */
  cta?: WorldSectionCta;
}

export interface WorldConfig {
  brand: { name: string; href: string };
  hint: string;
  diveScroll: number;
  connScroll: number;
  sections: WorldSection[];
  connectors: string[];
  connectorsMobile: string[];
}

// Same GitHub repo URL used on the home page finale (src/content/home/body.html).
const GITHUB_URL = 'https://github.com/saeedkolivand/ai-job-hunter-app';

export const WORLD_CONFIG: WorldConfig = {
  brand: { name: 'AI Job Hunter', href: '/' },
  hint: 'scroll to fly in',
  diveScroll: 1.3,
  connScroll: 0.9,
  sections: [
    {
      id: 'slump',
      label: 'The Slump',
      still: '/world/slump.webp',
      stillMobile: '/world/slump-m.webp',
      clip: '/world/vid/slump.mp4',
      clipMobile: '/world/vid/slump-m.mp4',
      accent: '#e24b4a',
      scroll: 1.6,
      linger: 0.45,
      eyebrow: 'THE PROBLEM',
      title: 'Job hunting broke me.',
      body: 'A thousand applications. Zero replies. One very dead plant.',
      tags: [],
    },
    {
      id: 'descent',
      label: 'The Doomscroll',
      still: '/world/descent.webp',
      stillMobile: '/world/descent-m.webp',
      clip: '/world/vid/descent.mp4',
      clipMobile: '/world/vid/descent-m.mp4',
      accent: '#6cc6ff',
      eyebrow: 'THE DOOMSCROLL',
      title: 'Every board. Every day. Nothing.',
      body: 'Scrolling 24 job boards until the towers all look the same.',
      tags: ['24 boards'],
    },
    {
      id: 'workshop',
      label: 'The Turn',
      still: '/world/workshop.webp',
      stillMobile: '/world/workshop-m.webp',
      clip: '/world/vid/workshop.mp4',
      clipMobile: '/world/vid/workshop-m.mp4',
      accent: '#e24b4a',
      eyebrow: 'THE TURN',
      title: 'So I built a robot to do it.',
      body: 'Cardboard, tape, spite, and one weekend that became six months.',
      tags: [],
    },
    {
      id: 'engine',
      label: 'The Robot',
      still: '/world/engine.webp',
      stillMobile: '/world/engine-m.webp',
      clip: '/world/vid/engine.mp4',
      clipMobile: '/world/vid/engine-m.mp4',
      accent: '#6cc6ff',
      eyebrow: 'THE ROBOT',
      title: 'It does everything else.',
      body: 'Scrapes the boards, writes the letters, scores your resume against the job.',
      tags: ['scrape', 'write', 'score'],
    },
    {
      id: 'godmode',
      label: 'Godmode',
      still: '/world/godmode.webp',
      stillMobile: '/world/godmode-m.webp',
      clip: '/world/vid/godmode.mp4',
      clipMobile: '/world/vid/godmode-m.mp4',
      accent: '#e24b4a',
      eyebrow: 'GODMODE',
      title: 'It hunts while you sleep.',
      body: 'Autopilot searches, a browser extension, everything local on your machine.',
      tags: ['autopilot', 'local-first'],
    },
    {
      id: 'offer',
      label: 'The Offer',
      still: '/world/offer.webp',
      stillMobile: '/world/offer-m.webp',
      clip: '/world/vid/offer.mp4',
      clipMobile: '/world/vid/offer-m.mp4',
      accent: '#e24b4a',
      scroll: 1.7,
      linger: 0.5,
      eyebrow: 'THE PAYOFF',
      title: 'ok fine, take the app',
      body: 'Free, open source, does everything but hit submit. That part is still you.',
      tags: [],
      cta: {
        primary: { label: 'Take the app', href: '/download' },
        secondary: { label: 'Star it on GitHub', href: GITHUB_URL },
      },
    },
  ],
  connectors: [
    '/world/vid/conn1.mp4',
    '/world/vid/conn2.mp4',
    '/world/vid/conn3.mp4',
    '/world/vid/conn4.mp4',
    '/world/vid/conn5.mp4',
  ],
  connectorsMobile: [
    '/world/vid/conn1-m.mp4',
    '/world/vid/conn2-m.mp4',
    '/world/vid/conn3-m.mp4',
    '/world/vid/conn4-m.mp4',
    '/world/vid/conn5-m.mp4',
  ],
};
