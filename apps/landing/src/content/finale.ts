// Human-visible copy for the FINALE section (honest paragraph, CTA, source
// links, funding, footnote, byline, footer nav). ASCII-only source: any
// non-ASCII glyph (em dash, arrow, play triangle, heart, middle dot, ellipsis)
// is \uXXXX-escaped.
export const finale = {
  screamLines: "we're still here?|ok, last one|\u2026just go apply",
  screamVoice: "smug",
  doodleAria: "poke the finale guy",
  honest:
    "yes, this app is real. yes, it actually pulls from 24 boards \u2014 LinkedIn over HTTP, the walled ones (Indeed, Glassdoor, StepStone, Xing, Workday) via the Adzuna/JSearch aggregator API so no one has to make a 9th Workday account, and a bunch of ATS and DACH boards on top. yes, it's built with Tauri + Rust + React 19 + a vector database + a pure-Rust Typst engine that renders every PDF \u2014 because I had a lot of free time, on account of the unemployment. no, it does not auto-apply \u2014 it finds the jobs and writes the whole application; hitting submit is the one job left to you. no, I still don't have a job. the autopilot is doing its best.",
  cta: "ok fine, take the app \u2192",
  srcGithub:
    "view the source \u2014 it's PolyForm Noncommercial: read it, fork it, learn from it. just don't sell my misery back to me.",
  srcCreature:
    "\u25b6 THE CREATURE \u2014 a hand-drawn doodle about the tiny recruiter you accidentally summon. it grows. (2:40)",
  fundPrefix: "or fund a man's job hunt \u2192 ",
  fundCoffee: "buy me a coffee",
  fundSponsor: "sponsor",
  fundPaypal: "PayPal",
  fundSep: " \u00b7 ",
  footnotePre:
    "macOS will say the app is \"damaged.\" it's not damaged. it's just unsigned \u2014 like a contract I was never offered. run ",
  footnoteCode: "xattr -cr",
  footnotePost: " and we move on.",
  builtwith: "Tauri \u00b7 Rust \u00b7 React 19 \u00b7 TanStack \u00b7 SQLite \u00b7 Typst \u00b7 Ollama \u00b7 pure spite",
  byline: "made by Saeed, between rejections.",
  footSep: " \u00b7 ",
  footHome: "home",
  footDownload: "download",
  footPrivacy: "privacy",
  footFilm: "\u25b6 the short film",
  footGithub: "GitHub",
  footChrome: "Chrome extension",
  footFirefox: "Firefox extension",
  footSponsor: "\u2665 sponsor",
};
