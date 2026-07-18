// The 9-page p-space registry (webgl-standards "The 9-page p-space model").
// Everything scroll-driven is a pure function of the global t in [0,1]. Page i
// is active when t is in [i/9, (i+1)/9]. Within a page, p in [0,1] maps that
// slice; p in [0, EXIT_START] plays the page and p in [EXIT_START, 1] scrubs
// its exit (a Rip for pages 1-7, the hinge for page 0, the signature for page
// 8). pageProgress is pure math -- exported for the shader chunk and unit tests.

export const PAGE_COUNT = 9;

// Play/exit split: p <= 0.72 plays, p > 0.72 scrubs the exit.
export const EXIT_START = 0.72;

// Physical page dimensions (world units), shared by the pre-split geometry and
// the camera framing so the notebook fills the desk view consistently.
export const PAGE_W = 2.2;
export const PAGE_H = 3.0;

export type PageExit =
  | "hinge"
  | "corner-tear"
  | "horizontal-rip"
  | "vertical-tear"
  | "crumple"
  | "paper-plane"
  | "diagonal-tear"
  | "perforation-zip"
  | "signature";

export interface PageDef {
  id: string;
  exit: PageExit;
}

// Order is load-bearing: index === page number. Exit styles are recorded now so
// later milestones plug their exit mesh in by id without renumbering.
export const PAGES: readonly PageDef[] = [
  { id: "cover", exit: "hinge" },
  { id: "slump", exit: "corner-tear" },
  { id: "descentA", exit: "horizontal-rip" },
  { id: "descentB", exit: "vertical-tear" },
  { id: "fried", exit: "crumple" },
  { id: "areYouSure", exit: "paper-plane" },
  { id: "features", exit: "diagonal-tear" },
  { id: "testimonials", exit: "perforation-zip" },
  { id: "godmode", exit: "signature" },
] as const;

// The one page M1 authors as a real tear mesh: the corner-tear "slump" page.
export const CORNER_TEAR_PAGE = 1;

function clamp01(v: number): number {
  return v < 0 ? 0 : v > 1 ? 1 : v;
}

// Which page owns t. floor(t * 9), clamped so t === 1 stays on the last page.
export function activePageFor(t: number): number {
  const i = Math.floor(clamp01(t) * PAGE_COUNT);
  return i >= PAGE_COUNT ? PAGE_COUNT - 1 : i < 0 ? 0 : i;
}

export interface PageProgress {
  // Progress through page i's whole scroll slice, [0,1].
  p: number;
  // Progress through page i's exit sub-slice, [0,1] (0 until p passes EXIT_START).
  exitP: number;
}

// Map global t to page i's local play/exit progress. Pure and idempotent for a
// given t -- the scrub-safety contract depends on it.
export function pageProgress(t: number, i: number): PageProgress {
  const p = clamp01(t * PAGE_COUNT - i);
  const exitP = p <= EXIT_START ? 0 : (p - EXIT_START) / (1 - EXIT_START);
  return { p, exitP };
}
