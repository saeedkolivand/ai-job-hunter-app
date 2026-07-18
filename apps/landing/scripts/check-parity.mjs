// Copy-parity gate: the Semantic layer (src/content/story.ts) must keep every
// legacy joke + link from landing/index.html, with no orphan links. Dependency-
// light (node stdlib only); exits nonzero on drift. Wired as `check:parity`.
//
// Two checks:
//   1. Bidirectional link diff -- every content anchor href in the legacy page
//      appears in story.ts, and story.ts introduces no href the legacy page
//      lacks.
//   2. Signature-phrase presence -- each curated ASCII joke must appear in BOTH
//      the legacy page and the semantic copy. The list lives here (not in the
//      content) so verifying a phrase in story.ts genuinely exercises the copy.

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const indexPath = join(here, "../../../landing/index.html");
const storyPath = join(here, "../src/content/story.ts");

const indexHtml = readFileSync(indexPath, "utf8");
const storySrc = readFileSync(storyPath, "utf8");

const PARITY_PHRASES = [
  "IT DOES EVERYTHING ELSE.",
  "we don't track you. we can barely track ourselves.",
  "Are you sure?",
  "these buttons are fake",
  "mute the guy",
  "rejections survived",
  "cover_letter_FINAL_v9",
  "is it normal to cry on a tuesday",
  "PolyForm Noncommercial",
  "xattr -cr",
  "still don't have a job",
  "runs 100% offline with Ollama",
  "Semantic matching",
  "ATS scoring",
  "Autopilot",
  "24 boards",
  "press send",
  "made by Saeed",
  "THE CREATURE",
  "please hire",
];

// Content links only: http(s) URLs and absolute route paths (/creature etc).
// In-page (#...) and bare "/" anchors are navigation, not content, and ignored.
function isContentHref(href) {
  return /^https?:\/\//.test(href) || /^\/[a-z]/i.test(href);
}

// Legacy content hrefs from <a href="..."> in the hand-authored page.
function anchorHrefs(html) {
  const set = new Set();
  const re = /<a\b[^>]*\bhref="([^"]+)"/gi;
  let m;
  while ((m = re.exec(html)) !== null) {
    if (isContentHref(m[1])) set.add(m[1]);
  }
  return set;
}

// Semantic-layer hrefs from `href: "..."` literals in story.ts.
function storyHrefs(src) {
  const set = new Set();
  const re = /\bhref:\s*"([^"]+)"/g;
  let m;
  while ((m = re.exec(src)) !== null) {
    if (isContentHref(m[1])) set.add(m[1]);
  }
  return set;
}

const legacy = anchorHrefs(indexHtml);
const semantic = storyHrefs(storySrc);
const errors = [];

for (const href of legacy) {
  if (!semantic.has(href)) errors.push(`link MISSING from Semantic layer: ${href}`);
}
for (const href of semantic) {
  if (!legacy.has(href)) errors.push(`link ORPHAN in Semantic layer (not in landing/index.html): ${href}`);
}

for (const phrase of PARITY_PHRASES) {
  if (!indexHtml.includes(phrase)) errors.push(`phrase not in landing/index.html: "${phrase}"`);
  if (!storySrc.includes(phrase)) errors.push(`phrase not in Semantic layer copy: "${phrase}"`);
}

if (errors.length > 0) {
  console.error("check:parity FAILED -- copy/link drift vs landing/index.html:");
  for (const e of errors) console.error(`  - ${e}`);
  console.error(
    `\n${errors.length} issue(s). Legacy content links: ${legacy.size}, semantic links: ${semantic.size}.`,
  );
  process.exit(1);
}

console.log(
  `check:parity OK -- ${legacy.size} legacy links present with no orphans; ` +
    `${PARITY_PHRASES.length} signature phrases intact.`,
);
