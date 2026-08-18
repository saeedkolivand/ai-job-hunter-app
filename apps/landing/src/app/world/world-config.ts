// Config for the /world scroll-scrubbed camera-flight route. Kept as plain,
// unit-testable data (world-config.test.ts) separate from the client component
// that mounts the vendored engine (WorldClient.tsx).
//
// Poster stills are absolute from the site root and ship in public/world/ —
// they are ~2 MB total and must paint before anything is fetched. The VIDEO
// clips do not ship in the repo: 33 files, 62 MB, and re-encoding them rewrites
// every byte, so a single re-encode round already cost ~38 MB of permanent
// history. They live in object storage and are addressed through `vid()`.

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

// Where the /world clips are served from: an R2 bucket behind
// cdn.aijobhunter.app, not the repo and not `public/`.
//
// Two reasons they are not in git. Video does not delta-compress, so every
// re-encode writes every byte again as permanent history; one re-encode round
// had already cost ~38 MB. And serving them from `public/` put them on the
// Pages site, spending its 100 GB/month soft bandwidth limit — about 5,000
// visits at this route's 20 MB device-set cap.
//
// Why R2 specifically, and not the GitHub release they briefly lived on:
// `scrub-engine.js` loads each clip with `fetch(url).then(r => r.blob())`, so
// the response MUST carry `Access-Control-Allow-Origin`. GitHub release
// downloads do not send it, and the route silently fell back to its poster
// stills. R2 lets the bucket declare a CORS policy; this origin is on it.
//
// Egress from R2 is unmetered, so the bandwidth ceiling is gone rather than
// moved. Objects are uploaded with `immutable` and a one-year max-age, which is
// honest because a new clip set means a new key, never a mutated one.
//
// Override for offline work: `NEXT_PUBLIC_WORLD_VIDEO_BASE=/world/vid` with the
// clips in `apps/landing/public/world/vid/` (gitignored). Next inlines this at
// build time, which is what the static export needs. localhost:3000 is on the
// bucket's CORS allowlist too, so plain `next dev` works without the override.
const VIDEO_BASE =
  process.env.NEXT_PUBLIC_WORLD_VIDEO_BASE ?? 'https://cdn.aijobhunter.app/world/vid';

/** URL for one /world clip. `file` is the bare name, e.g. `slump.mp4`. */
const vid = (file: string): string => `${VIDEO_BASE}/${file}`;

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
      clip: vid('slump.mp4'),
      clipMobile: vid('slump-m.mp4'),
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
      clip: vid('descent.mp4'),
      clipMobile: vid('descent-m.mp4'),
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
      clip: vid('workshop.mp4'),
      clipMobile: vid('workshop-m.mp4'),
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
      clip: vid('engine.mp4'),
      clipMobile: vid('engine-m.mp4'),
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
      clip: vid('godmode.mp4'),
      clipMobile: vid('godmode-m.mp4'),
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
      clip: vid('offer.mp4'),
      clipMobile: vid('offer-m.mp4'),
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
    vid('conn1.mp4'),
    vid('conn2.mp4'),
    vid('conn3.mp4'),
    vid('conn4.mp4'),
    vid('conn5.mp4'),
  ],
  connectorsMobile: [
    vid('conn1-m.mp4'),
    vid('conn2-m.mp4'),
    vid('conn3-m.mp4'),
    vid('conn4-m.mp4'),
    vid('conn5-m.mp4'),
  ],
};

const toAv1 = (path: string): string => path.replace(/\.mp4$/, '.av1.mp4');

/**
 * Returns a copy of `config` pointing desktop clips (section.clip, connectors)
 * at their AV1 encode instead of H.264. Mobile stays H.264-only — untouched:
 * clipMobile, connectorsMobile, still, and stillMobile. Pure — never mutates
 * `config`; no DOM access (the AV1-support check lives in WorldClient.tsx).
 */
export function withAv1Sources(config: WorldConfig): WorldConfig {
  return {
    ...config,
    sections: config.sections.map((section) => ({ ...section, clip: toAv1(section.clip) })),
    connectors: config.connectors.map(toAv1),
  };
}
