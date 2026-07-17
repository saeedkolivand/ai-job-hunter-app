"use client";

// Debug smoke-test surface for the self-hosted GL fonts. Reached only via the
// ?fonts=1 query param (see Experience) -- never part of a normal build, no
// route. Renders one drei <Text> per family using the page's real copy so the
// gate audit can screenshot every typeface at once and confirm: local TTFs
// load, accented glyphs (resume-with-acute), quotes and digits shape correctly,
// emoji/star scrub swaps in DRAWN_GLYPH instead of tofu.

import { useEffect } from "react";
import { Text } from "@react-three/drei";

import { beat2 } from "@/content/beat2";
import { features } from "@/content/features";
import { hero } from "@/content/hero";
import { testimonials } from "@/content/testimonials";

import { charactersFor, FONT, FONT_TEXTS, type FontKey, scrubEmoji } from "./text";

// Each row: the family (by key, so `characters` can pull FONT_TEXTS[key] --
// the same union GLLoader preloads) + a real copy snippet to actually render.
// Snippets are the same ASCII-escaped strings the semantic/GL copy uses;
// scrubEmoji runs on each so the party-popper and star rows demonstrate the
// DRAWN_GLYPH swap.
const ROWS: { key: FontKey; text: string }[] = [
  { key: "impact", text: hero.h1a }, // Anton
  { key: "scrawl", text: hero.scrollhint }, // Gloria Hallelujah
  { key: "hand", text: features.c2t }, // Patrick Hand -- accented copy
  { key: "caveat", text: hero.dontClick }, // Caveat -- apostrophe
  { key: "mono", text: features.c1t }, // Space Mono 400 -- digits
  { key: "monoBold", text: beat2.blackholeMain }, // Space Mono 700 -- quotes
  { key: "hand", text: beat2.feed[0] }, // party popper -> DRAWN_GLYPH
  { key: "mono", text: testimonials.stars }, // star run -> DRAWN_GLYPH
];

export default function FontsDebug({ onReady }: { onReady: () => void }) {
  // No positive-priority useFrame here (that would suppress R3F's auto-render),
  // so lift the boot overlay off a rAF once the tree has mounted instead.
  // LoaderLift (Experience's normal-mode overlay lift) isn't mounted in fonts
  // mode, so this is the only thing that clears #loader here.
  useEffect(() => {
    const id = requestAnimationFrame(() => {
      document.getElementById("loader")?.classList.add("gone");
      onReady();
    });
    return () => cancelAnimationFrame(id);
  }, [onReady]);

  const top = 2.1;
  const step = 0.56;

  return (
    <group>
      {ROWS.map((row, i) => {
        const text = scrubEmoji(row.text);
        return (
          <Text
            key={i}
            font={FONT[row.key]}
            characters={charactersFor(...FONT_TEXTS[row.key])}
            position={[0, top - i * step, 0]}
            fontSize={0.18}
            maxWidth={8}
            anchorX="center"
            anchorY="middle"
            textAlign="center"
            color="#f4ecdc"
          >
            {text}
          </Text>
        );
      })}
    </group>
  );
}
