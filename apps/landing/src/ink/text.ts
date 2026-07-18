// GL text plumbing. All landing text renders in GL (drei/troika <Text>), which
// is TTF-only and needs three things wired consistently: the self-hosted font
// URLs, a pre-mount atlas warm-up, and an explicit `characters` union per
// <Text> (troika silently drops glyphs with no characters prop). This module
// is the single source for all three.
//
// Fonts are the same local TTFs the semantic layer's @font-face rules load
// (see app/globals.css) -- served from /public/fonts, zero Google CDN hits.

import { preloadFont } from "troika-three-text";

import { beat1 } from "@/content/beat1";
import { beat2 } from "@/content/beat2";
import { beat3 } from "@/content/beat3";
import { beat4 } from "@/content/beat4";
import { features } from "@/content/features";
import { finale } from "@/content/finale";
import { hero } from "@/content/hero";
import { testimonials } from "@/content/testimonials";

// Family -> self-hosted TTF url. Keys match the CSS var family names in
// globals.css so GL and DOM render the same typefaces. Caveat is a variable
// font (one file spans the 600/700 the copy uses); troika renders its default
// instance, which is enough for the SDF outlines.
export const FONT = {
  scrawl: "/fonts/GloriaHallelujah.ttf", // Gloria Hallelujah -- handwritten scrawl
  hand: "/fonts/PatrickHand-Regular.ttf", // Patrick Hand -- body hand copy
  impact: "/fonts/Anton-Regular.ttf", // Anton -- impact headlines
  caveat: "/fonts/Caveat-VariableWght.ttf", // Caveat -- accent script
  mono: "/fonts/SpaceMono-Regular.ttf", // Space Mono 400
  monoBold: "/fonts/SpaceMono-Bold.ttf", // Space Mono 700
} as const;

export type FontKey = keyof typeof FONT;

// P2 gate note: the ~1s-after-boot WebGL2 context loss originally blamed on
// troika's SDF WebGL1 work was actually R3F force-losing its own context after
// a drei <Text> suspension bubbled out of the Canvas -- see the Suspense
// boundary comment in experience/Experience.tsx for the traced root cause.
// Troika's atlas/SDF WebGL1 usage (including its GPU trial probe) was observed
// running to completion with zero fallout, so it is left at its defaults.

// P2 gate: no emoji in GL text. troika has no color-emoji support, so the
// party-popper (content beat2) and star run (content testimonials) must not be
// fed to a GL <Text>. Any GL-bound copy that would include them substitutes
// this marker; a drawn ink glyph replaces it in a later ink pass.
// ponytail: plain placeholder string, no glyph atlas. Ceiling -- shows literal
// text if a scene forgets to swap it. Upgrade path: real Line2 ink glyph in the
// stroke pass, then delete this constant and its scrubEmoji callers.
export const DRAWN_GLYPH = "[*]"; // TODO(ink): replace with a drawn Line2 glyph

// Strip glyphs troika can't render (emoji + the decorative star run) out of any
// string headed for a GL <Text>, swapping in DRAWN_GLYPH so nothing renders as
// tofu. Astral emoji (surrogate pairs) and BMP stars (\u2605/\u2606) covered.
const GL_UNSUPPORTED = /(?:[\uD800-\uDBFF][\uDC00-\uDFFF]|[\u2605\u2606]+)/g;
export function scrubEmoji(s: string): string {
  return s.replace(GL_UNSUPPORTED, DRAWN_GLYPH);
}

// Build the union set of every glyph a group of strings needs, as a single
// string for the troika `characters` prop. Deduped and sorted for a stable
// atlas across renders. Emoji/stars are scrubbed first so they never enter the
// atlas request.
export function charactersFor(...strings: string[]): string {
  const set = new Set<string>();
  for (const raw of strings) {
    for (const ch of scrubEmoji(raw)) set.add(ch);
  }
  return Array.from(set).sort().join("");
}

// Every string any scene (or the ?fonts=1 debug grid) actually renders through
// a given family. Single source so GLLoader's preload phase and the scenes
// that render <Text> never drift apart -- scenes should build their
// `characters` prop from this, not from ad hoc strings.
export const FONT_TEXTS: Record<FontKey, string[]> = {
  impact: [hero.h1a, beat2.blackholeYell, beat3.huge1, beat3.huge2, beat3.huge3],
  scrawl: [
    hero.scrollhint,
    beat1.sectionLabel,
    beat2.h2,
    features.h2,
    testimonials.heading,
    testimonials.headingSmall,
    testimonials.starsWho,
    ...testimonials.quotes.map((q) => q.who),
  ],
  hand: [
    ...Object.values(features),
    beat2.feed[0],
    hero.sub,
    hero.subBold,
    beat1.thought,
    beat1.counterA,
    beat1.counterB,
    beat1.counterC,
    beat2.linkedinTitle,
    beat4.big,
    beat4.counter.p1,
    beat4.counter.p2,
    beat4.counter.p3,
    beat4.line1,
    beat4.line2a,
    beat4.line2b,
    beat3.mid,
    ...Object.values(beat3.line),
    "*****", // testimonials star rating -- ASCII asterisks stand in for the star
    ...testimonials.quotes.map((q) => q.quote),
    "0123456789",
    finale.cta, // CTA label chars (its U+2192 has no Patrick Hand glyph -- dropped)
    "->", // the ASCII arrow the button actually renders
  ],
  caveat: [hero.dontClick, hero.h1a, hero.h1b, hero.h1ul, hero.h1c],
  mono: [
    hero.kicker.toUpperCase(),
    features.c1t,
    testimonials.stars,
    testimonials.featuredPrefix,
    ...testimonials.featured,
    testimonials.sep,
    ...beat1.screencaps,
    beat2.atsBold,
    beat2.recruitersTitle,
    beat2.chipsMore,
    ...beat2.chips,
    beat3.dq,
    beat3.yes,
    beat3.yes2,
    finale.honest,
    finale.srcGithub,
    finale.builtwith,
    finale.byline,
  ],
  monoBold: [beat2.blackholeMain],
};

// Boot-quality warm-up (not a crash fix -- see Experience.tsx for that): fetch
// and atlas every family via troika's public preloadFont() BEFORE the GL
// Experience mounts, while #loader is still up. With the atlas warm, each drei
// <Text>'s own internal preloadFont-suspension resolves near-instantly and the
// hero copy is present on the first painted frame instead of popping in.
// sdfGlyphSize pinned to 64 = drei <Text>'s hardcoded default = troika's
// CONFIG default, so this warms the exact atlas bucket every path hits and no
// throwaway second atlas is ever built.
// Single-flight-guarded: Strict Mode double-invokes GLLoader's effect, and a
// second overlapping call must reuse the same in-flight promise rather than
// dispatch a second round of preloadFont requests -- otherwise Experience can
// mount (on the second call's early resolve) while the first call's glyphs
// are still draining through troika's shared macrotask queue.
// Each font resolves independently, and a preloadFont call that throws
// resolves its own slot instead of poisoning the batch; a font whose network
// fetch stalls still leaves its promise pending -- GLLoader races the whole
// batch against a timeout for that case.
let preloadPromise: Promise<void> | null = null;
export function preloadAllFonts(): Promise<void> {
  if (preloadPromise) return preloadPromise;
  const keys = Object.keys(FONT) as FontKey[];
  preloadPromise = Promise.all(
    keys.map(
      (key) =>
        new Promise<void>((resolve) => {
          try {
            preloadFont(
              { font: FONT[key], characters: charactersFor(...FONT_TEXTS[key]), sdfGlyphSize: 64 },
              () => resolve(),
            );
          } catch {
            resolve();
          }
        }),
    ),
  ).then(() => undefined);
  return preloadPromise;
}
