// Post-build merge: copy the hand-authored static pages from landing/ into this
// app's export output so out/ is always deploy-shaped (creature.html, privacy.html,
// benchmarks/, CNAME, og-card.jpg, ...). index.html is the ONE exclusion: Next owns
// the home page, so the legacy landing/index.html must never overwrite it.
//
// Runs as `next build`'s postbuild step. No-op-safe: if out/ is missing (build
// skipped or failed) it prints a message and exits 0 instead of throwing.

import { cpSync, existsSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const srcDir = join(here, "../../../landing");
const outDir = join(here, "../out");

if (!existsSync(outDir)) {
  console.warn("[merge-passthrough] out/ not found -- skipping (run `next build` first).");
  process.exit(0);
}

if (!existsSync(srcDir)) {
  console.warn("[merge-passthrough] landing/ source not found -- nothing to merge.");
  process.exit(0);
}

cpSync(srcDir, outDir, {
  recursive: true,
  // Skip ONLY the top-level index.html; benchmarks/index.html is preserved.
  filter: (src) => relative(srcDir, src) !== "index.html",
});

console.log("[merge-passthrough] merged landing/ passthrough pages into out/.");
