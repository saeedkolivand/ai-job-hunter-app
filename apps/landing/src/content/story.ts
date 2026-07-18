// The copy + link single source of truth for TERMINAL VELOCITY's Semantic layer.
// The SemanticLayer component renders this into crawlable HTML; the copy-parity
// script (scripts/check-parity.mjs) reads this file's text and diffs it against
// landing/index.html so every legacy joke + link keeps a diegetic home and no
// orphan links appear.
//
// ASCII-ONLY (Turbopack sourcemap rule): use "--" for dashes; any glyph that
// truly needs a non-ASCII code point must be \uXXXX-escaped. Signature joke
// phrases are kept verbatim ASCII so parity can substring-match them.

export interface StoryLink {
  href: string;
  label: string;
  external: boolean;
}

export interface StoryScene {
  id: string; // matches scene-resolver id -> hash-anchor deep-link target
  act: string;
  timecode: string; // display start time
  heading: string;
  copy: string[];
  links: StoryLink[];
}

export interface FeatureItem {
  title: string;
  body: string;
}

export const SITE = {
  name: "AI Job Hunter",
  tagline: "please hire him",
  description:
    "A realistic scroll-film about a burned-out job hunter and the robot that does everything but press send. A real desktop app. Also a cry for help.",
  url: "https://aijobhunter.app/",
};

// The projector-slate menu chip (reachable at any scroll position): download,
// GitHub, privacy, the creature. Mirrored into the a11y overlay while GL runs.
export const MENU_LINKS: StoryLink[] = [
  { href: "/download", label: "download", external: false },
  { href: "https://github.com/saeedkolivand/ai-job-hunter-app", label: "GitHub", external: true },
  { href: "/privacy", label: "privacy", external: false },
  { href: "/creature", label: "the creature", external: false },
];

export const SCENES: StoryScene[] = [
  {
    id: "cold-open",
    act: "Cold open",
    timecode: "00:00",
    heading: "2:47 AM.",
    copy: [
      "A live monitor glows in the dark. cover_letter_FINAL_v9.docx is still open on the last line he typed: please. please. please. pl--",
      "One tab asks how to answer what is your biggest weakness. Another asks: is it normal to cry on a tuesday. The chair tips past balance and the floor dissolves. please hire him.",
    ],
    links: [
      {
        href: "/creature",
        label: "or don't scroll -- watch THE CREATURE (2:40)",
        external: false,
      },
      { href: "/download", label: "already sold? just take the app", external: false },
    ],
  },
  {
    id: "the-canyon",
    act: "The canyon",
    timecode: "00:08",
    heading: "The canyon of rejection towers.",
    copy: [
      "He falls backward, slow-mo, down a canyon of glowing rejection towers. The falling paper is signage from 24 boards -- LinkedIn, Indeed, Glassdoor, Workday and the rest -- thickening into a storm.",
      "Behind the tower glass an ATS robot beeps REJECTED at a resume no human will read. An elevator indicator counts rejections survived as he drops: applications 1,000, responses 0, dignity offline.",
    ],
    links: [],
  },
  {
    id: "the-surface",
    act: "The surface",
    timecode: "00:48",
    heading: "The surface.",
    copy: [
      "He hits the paper ocean. The one hard beat: the letterbox flexes, sound cuts to silence, a splash crown rises around him.",
      "This is the loudest silence in the film. You can mute the guy entirely if the muttering is too much.",
    ],
    links: [],
  },
  {
    id: "the-deep",
    act: "The deep",
    timecode: "01:00",
    heading: "The deep.",
    copy: [
      "Underwater now. God-rays thin band by band as he sinks past the light. He goes limp. This is the saddest frame in the whole picture.",
    ],
    links: [],
  },
  {
    id: "blackout",
    act: "Blackout",
    timecode: "01:23",
    heading: "Blackout.",
    copy: [
      "Near-total dark. Only breathing. Then, far below, a single amber point of light appears and holds.",
    ],
    links: [],
  },
  {
    id: "the-catch",
    act: "The catch",
    timecode: "01:33",
    heading: "The catch.",
    copy: [
      "A submersible drone rises out of the black and catches him, gently, at the bottom.",
      "Its lens HUD asks: Are you sure? yes / YES. Nothing happens -- these buttons are fake. It does everything else; the one thing it will never do is press send. That terror stays yours.",
    ],
    links: [],
  },
  {
    id: "the-ascent",
    act: "The ascent",
    timecode: "01:42",
    heading: "The ascent.",
    copy: [
      "The axis inverts. The robot carries him up the same water he descended. Falling paper folds into paper planes and flies in formation. He sleeps in its arms.",
    ],
    links: [],
  },
  {
    id: "dawn",
    act: "Dawn",
    timecode: "02:16",
    heading: "Dawn.",
    copy: [
      "They break the surface into a flat calm at sunrise. The first warm, full-color frame of the film. He is still unemployed. He is also, for the first time, rested.",
    ],
    links: [],
  },
  {
    id: "finale",
    act: "Finale / credits",
    timecode: "02:32",
    heading: "The one real action.",
    copy: [
      "The robot surfaces holding one red SEND button. It is the single real action in the whole film -- the product's thesis. The credits roll. Somewhere, a tiny recruiter you accidentally summoned is still growing.",
    ],
    links: [{ href: "/creature", label: "THE CREATURE -- the short film (2:40)", external: false }],
  },
];

// The end-credits roll: the full footer. Carries the honest paragraph, the
// feature list, and every legacy link as a real crawlable anchor.
export const CREDITS = {
  tagline: "IT DOES EVERYTHING ELSE.",
  honest:
    "yes, this app is real. it pulls from 24 boards, writes the whole tailored application, and leaves exactly one job to you: press send. no, it does not auto-apply. no, I still don't have a job. the autopilot is doing its best.",
  privacy: "we don't track you. we can barely track ourselves.",
  features: [
    {
      title: "24 boards, one search",
      body: "LinkedIn direct; the walled boards via an aggregator API -- no new Workday account -- plus the Greenhouse / Lever / Ashby / DACH lineup on top.",
    },
    {
      title: "AI cover letters and resumes",
      body: "It writes them for you across 12 templates, rendered by a pure-Rust Typst engine to DOCX, PDF and TXT.",
    },
    {
      title: "ATS scoring",
      body: "Your resume was judged by regex that has never felt joy. Now you score back.",
    },
    {
      title: "Semantic matching",
      body: "Hybrid vector search that understands your resume better than your mother, who still thinks you do computers.",
    },
    {
      title: "Autopilot",
      body: "Pick a board and a schedule and walk away. It scrapes, ranks, pings you, and pre-writes each application. The submit button it leaves to you.",
    },
    {
      title: "Local, cloud, or CLI agents",
      body: "runs 100% offline with Ollama, or drop in your own OpenAI / Anthropic / Gemini key, or route through a CLI agent you already pay for.",
    },
    {
      title: "Browser extension (save jobs one-click)",
      body: "One click imports a posting straight into the desktop app.",
    },
  ] as FeatureItem[],
  extensionLinks: [
    {
      href: "https://chromewebstore.google.com/detail/ai-job-hunter-%E2%80%94-job-impor/oaoekkgkhmgdfnpmfkpphgiikliaicll",
      label: "Chrome extension",
      external: true,
    },
    {
      href: "https://addons.mozilla.org/en-US/firefox/addon/ai-job-hunter-job-importer/",
      label: "Firefox extension",
      external: true,
    },
  ] as StoryLink[],
  macNote:
    "macOS will say the app is damaged. it is not damaged, just unsigned -- like a contract I was never offered. run xattr -cr and we move on.",
  builtWith: "Tauri . Rust . React 19 . TanStack . SQLite . Typst . Ollama . pure spite",
  byline: "made by Saeed, between rejections.",
  // The finale link roll.
  links: [
    { href: "/download", label: "ok fine, take the app", external: false },
    {
      href: "https://github.com/saeedkolivand/ai-job-hunter-app",
      label: "view the source -- it's PolyForm Noncommercial: read it, fork it, learn from it",
      external: true,
    },
    { href: "https://ko-fi.com/saeedkolivand", label: "buy me a coffee", external: true },
    { href: "https://github.com/sponsors/saeedkolivand", label: "sponsor", external: true },
    { href: "https://paypal.me/saeedkolivand", label: "PayPal", external: true },
  ] as StoryLink[],
  // Foot navigation (home is label-only, not a link -- it is the current page).
  footNav: [
    { href: "/download", label: "download", external: false },
    { href: "/privacy", label: "privacy", external: false },
    { href: "/creature", label: "the short film", external: false },
    {
      href: "https://github.com/saeedkolivand/ai-job-hunter-app",
      label: "GitHub",
      external: true,
    },
    {
      href: "https://chromewebstore.google.com/detail/ai-job-hunter-%E2%80%94-job-impor/oaoekkgkhmgdfnpmfkpphgiikliaicll",
      label: "Chrome extension",
      external: true,
    },
    {
      href: "https://addons.mozilla.org/en-US/firefox/addon/ai-job-hunter-job-importer/",
      label: "Firefox extension",
      external: true,
    },
    {
      href: "https://github.com/sponsors/saeedkolivand",
      label: "sponsor",
      external: true,
    },
  ] as StoryLink[],
};
