// The GSAP -> GL bridge. One preallocated channel object per page, written by
// the single scrub master timeline (scroll.ts) with ABSOLUTE tweens and read by
// reference inside the composer's useFrame. This array is allocated exactly
// once, at module load; nothing pushes/splices or replaces an entry afterwards,
// so the per-frame GL path never allocates. p/exitP mirror pages.pageProgress
// (p = whole-slice progress, exitP = exit sub-slice progress) but are driven by
// GSAP so a single timeline owns all page motion.

import { PAGE_COUNT } from "./pages";

export interface PageChannel {
  p: number;
  exitP: number;
}

export const channels: PageChannel[] = Array.from(
  { length: PAGE_COUNT },
  () => ({ p: 0, exitP: 0 }),
);
