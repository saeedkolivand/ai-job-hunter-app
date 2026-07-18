// Deterministic store-asset pipeline for the AI Job Hunter browser extension.
//
// Generates Chrome Web Store assets via headless Chromium (Playwright), using
// the landing page's real brand language (paper/ink/red + hand-drawn doodles).
// No AI image generation: every asset is composited with HTML/CSS so text is
// crisp and the final PNG dimensions are exact.
//
// Pipeline:
//   1. Capture the REAL built popup (dist/chrome/popup.html) in 5 states
//      (offline / pairing / connected / fill / answers). popup.js is BLOCKED so
//      it can't force the offline view after its 3s timeout; popup.css loads
//      normally; each state is forced by direct DOM manipulation.
//   2. Composite 5 doodle-annotated 1280x800 store screenshots (Chrome caps the
//      listing at 5): private → pair → import → autofill → answer tools.
//   3. Render a 440x280 promo tile + a 1400x560 marquee.
//
// Run: node apps/extension/scripts/gen-store-assets.mjs
// (run from anywhere — paths are resolved relative to this file)

import { fileURLToPath, pathToFileURL } from 'node:url';
import { readFileSync } from 'node:fs';
import path from 'node:path';

import { chromium } from 'playwright';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const EXT_ROOT = path.resolve(__dirname, '..');
const DIST_POPUP = path.join(EXT_ROOT, 'dist', 'chrome', 'popup.html');
const DIST_CSS = path.join(EXT_ROOT, 'dist', 'chrome', 'popup.css');
const ICON_128 = path.join(EXT_ROOT, 'src', 'icons', 'icon-128.png');
const OUT_SHOTS = path.join(EXT_ROOT, 'store-assets', 'screenshots');
const OUT_RAW = path.join(OUT_SHOTS, 'raw');
const OUT_PROMO = path.join(EXT_ROOT, 'store-assets', 'promo');

// Crisp output: render at 2x then the saved PNG is the target CSS size x2.
// To keep the FINAL files EXACTLY the target pixel dims we set the viewport to
// HALF the target and deviceScaleFactor:2 -> output = (target/2) * 2 = target.
const DSF = 2;

// ---- Brand tokens (lifted from landing/index.html :root) --------------------
const PAPER = '#f4ecdc';
const INK = '#1c1812';
const RED = '#e24b4a';

// Google Fonts link — identical family set to the landing.
const FONTS_LINK = `
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Anton&family=Gloria+Hallelujah&family=Patrick+Hand&family=Space+Mono:wght@400;700&display=swap" rel="stylesheet">`;

const FONT_VARS = `
  --scrawl:'Gloria Hallelujah', cursive;
  --hand:'Patrick Hand', cursive;
  --impact:'Anton', Impact, sans-serif;
  --mono:'Space Mono', monospace;
  --paper:${PAPER}; --ink:${INK}; --red:${RED};`;

// Film-grain overlay — the landing's body::after feTurbulence data-URI.
const GRAIN_DATAURI =
  "url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='120' height='120'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2'/></filter><rect width='120' height='120' filter='url(%23n)'/></svg>\")";

const GRAIN_CSS = `
  .grain{content:""; position:absolute; inset:0; z-index:60; pointer-events:none; opacity:.06;
    background-image:${GRAIN_DATAURI}; mix-blend-mode:overlay;}`;

// Hand-drawn red arrow — the landing's .dc-arrow path, recoloured to --red.
// viewBox 0 0 50 44: shaft curves from upper-right (46,10) down to the tip
// (8,32); two short strokes form the arrowhead at the tip.
// Used ONLY by the marquee accent (a decorative flourish, not part of the
// 5-screenshot set — those use buildShotArrow(), computed per shot from the
// measured caption bbox and target button).
function redArrow({ rotate = 0, flip = false, width = 150 } = {}) {
  const sx = flip ? -1 : 1;
  return `<svg class="arrow" viewBox="0 0 50 44" style="width:${width}px;transform:rotate(${rotate}deg) scaleX(${sx});">
    <path d="M46 10 Q22 10 8 32" fill="none" stroke="${RED}" stroke-width="3.6" stroke-linecap="round"/>
    <path d="M8 32 l13 -2 M8 32 l4 -13" fill="none" stroke="${RED}" stroke-width="3.6" stroke-linecap="round"/>
  </svg>`;
}

// Hand-drawn red pointer arrow for the 5 store screenshots — computed PER SHOT
// from the actually-rendered geometry, as a full-frame SVG overlay in 1280x800
// frame coordinates. Nothing about the arrow is shared between shots:
//
//   tail  — anchored to the caption: the composite page is rendered first
//           WITHOUT the arrow, the caption element's bbox is measured live,
//           and the tail starts just off its nearest edge (below the last
//           line, or trailing the right edge — whichever is closer to the
//           target without degenerating into a stub or a near-vertical drop).
//   tip   — a small standoff (TIP_GAP) outside the card's LEFT edge, level
//           with the measured target button's centre (capture-time fy/fh
//           fractions, rotated with the card). It never crosses the card face.
//   curve — one cubic whose departure control derives from the tail→tip
//           vector (it bows AWAY from the caption side, curvature scales with
//           distance, and it flips naturally when the target sits above vs
//           below the caption) and whose ARRIVAL control is constrained so the
//           path arrives pointing at the target button's measured centre. A
//           deterministic solver grows the bow until the whole stroke clears
//           the caption bbox by ARROW.captionPad (asserted — the stroke must
//           never overlap caption glyphs).
//   look  — seeded per-shot jitter (tail spot, bow, wobble, stroke width,
//           barb length/spread) so each arrow reads hand-drawn and no two are
//           identical. Brand look preserved: single red stroke, round caps,
//           two-barb arrowhead rotated to the cubic's ANALYTIC tangent at the
//           tip (tip − arrival control) — never a hardcoded head angle.
//
// Rendered as the LAST element in the stage with the highest z-index, so the
// head is never occluded by the popup image.
const ARROW = {
  strokeBase: 5, // hand-drawn weight in frame px (+ seeded jitter)
  barbBase: 26, // arrowhead barb length in frame px (± seeded jitter)
  tailMargin: 30, // gap between the caption bbox edge and the tail start
  captionPad: 12, // asserted stroke clearance around the caption bbox
  shadowPad: 4, // drop-shadow spill included in the clearance check
};

// Deterministic per-shot PRNG (FNV-1a seed + mulberry32): same file name in,
// same arrow out — the pipeline stays reproducible, but every shot gets its
// own hand-drawn character.
function seededRng(str) {
  let h = 2166136261;
  for (let i = 0; i < str.length; i++) {
    h ^= str.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  let s = h >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = Math.imul(s ^ (s >>> 15), 1 | s);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// True if any sampled stroke point, inflated by `inflate` (half stroke width +
// shadow spill), lands inside the caption bbox padded by ARROW.captionPad —
// i.e. the arrow would crowd the caption glyphs.
function arrowCrowdsCaption(points, inflate, cap) {
  const pad = ARROW.captionPad;
  const x0 = cap.x - pad;
  const y0 = cap.y - pad;
  const x1 = cap.x + cap.w + pad;
  const y1 = cap.y + cap.h + pad;
  return points.some(
    (p) => p.x + inflate > x0 && p.x - inflate < x1 && p.y + inflate > y0 && p.y - inflate < y1
  );
}

// Build the full-frame arrow SVG for one shot from measured geometry: `tip` +
// `aim` (frame px, from shotCardGeometry — the standoff point and the button
// centre the head must point at) and `cap` (the caption bbox in frame px,
// measured in the live composite page). Returns the SVG string plus the
// resolved geometry for logging. Throws if no curve can clear the caption.
function buildShotArrow(shot, tip, aim, cap) {
  const rng = seededRng(shot.file);
  const stroke = ARROW.strokeBase + rng() * 1.4;
  const inflate = stroke / 2 + ARROW.shadowPad;

  // Tail candidates hang just off the caption bbox: below its last line, or
  // trailing its right edge. Pick whichever is closer to the tip, skipping
  // candidates that would degenerate (a stub-length shaft, or a near-vertical
  // drop whose head could not read as pointing AT the card).
  const below = {
    mode: 'below',
    x: cap.x + cap.w * (0.5 + rng() * 0.2),
    y: cap.y + cap.h + ARROW.tailMargin,
  };
  const trailing = {
    mode: 'right',
    x: cap.x + cap.w + ARROW.tailMargin,
    y: cap.y + cap.h * (0.4 + rng() * 0.25),
  };
  const dist = (p) => Math.hypot(tip.x - p.x, tip.y - p.y);
  const usable = [below, trailing].filter((p) => dist(p) >= 140 && Math.abs(tip.x - p.x) >= 120);
  const pool = usable.length > 0 ? usable : [below];
  const tail = pool.reduce((a, b) => (dist(a) <= dist(b) ? a : b));

  // Departure control from the tail→tip vector: a perpendicular offset at the
  // midpoint, bowed AWAY from the caption (the normal side pointing away from
  // the caption centre), curvature scaled by shaft length. The flip when the
  // target sits above vs below the caption falls out of the away-side
  // selection.
  const dx = tip.x - tail.x;
  const dy = tip.y - tail.y;
  const len = Math.hypot(dx, dy) || 1;
  const nx = -dy / len;
  const ny = dx / len;
  const mx = (tail.x + tip.x) / 2;
  const my = (tail.y + tip.y) / 2;
  const capCx = cap.x + cap.w / 2;
  const capCy = cap.y + cap.h / 2;
  const away =
    Math.hypot(mx + nx - capCx, my + ny - capCy) >= Math.hypot(mx - nx - capCx, my - ny - capCy)
      ? 1
      : -1;
  let bow = len * (0.12 + rng() * 0.08);

  // Arrival constraint: the cubic's tangent at t=1 is tip − c2, so placing c2
  // on the ray from the tip AWAY from the aim point makes the head arrive
  // pointing straight at the target button's centre.
  const aimLen = Math.hypot(aim.x - tip.x, aim.y - tip.y) || 1;
  const ax = (aim.x - tip.x) / aimLen;
  const ay = (aim.y - tip.y) / aimLen;
  const arriveLen = len * (0.3 + rng() * 0.1);
  const c2 = { x: tip.x - ax * arriveLen, y: tip.y - ay * arriveLen };

  // Hand wobble: a smooth seeded waver along the chord normal, zero at both
  // endpoints so the tail anchor and the tip stay exact.
  const wobbleAmp = 1.4 + rng() * 1.2;
  const wobbleFreq = 1.5 + rng() * 1.5;
  const wobblePhase = rng() * Math.PI * 2;
  const spread = ((26 + rng() * 6) * Math.PI) / 180;
  const barb = ARROW.barbBase - 3 + rng() * 6;

  // Arrowhead: two barbs swept back from the tip along the cubic's analytic
  // tangent at t=1 (∝ tip − c2 = the aim direction) — never hardcoded.
  const ang = Math.atan2(ay, ax);
  const b1 = {
    x: tip.x - barb * Math.cos(ang - spread),
    y: tip.y - barb * Math.sin(ang - spread),
  };
  const b2 = {
    x: tip.x - barb * Math.cos(ang + spread),
    y: tip.y - barb * Math.sin(ang + spread),
  };

  // Deterministic clearance solver: grow the bow away from the caption until
  // every sampled stroke point — shaft AND arrowhead barbs — clears the
  // caption bbox by ARROW.captionPad. The departure control (c1, derived from
  // the quadratic-equivalent bow point) is clamped left of the card edge so
  // the shaft never bulges over the card face; c2 is fixed by the arrival
  // constraint and does not move.
  const N = 32;
  let geom = null;
  for (let iter = 0; iter < 12 && !geom; iter++) {
    const qx = Math.min(mx + away * nx * bow, CARD_LEFT_EDGE - 24);
    const qy = my + away * ny * bow;
    const c1 = { x: tail.x + (2 / 3) * (qx - tail.x), y: tail.y + (2 / 3) * (qy - tail.y) };
    const shaft = [];
    for (let i = 0; i <= N; i++) {
      const t = i / N;
      const u = 1 - t;
      const w =
        Math.sin(Math.PI * t) * wobbleAmp * Math.sin(wobbleFreq * 2 * Math.PI * t + wobblePhase);
      shaft.push({
        x:
          u * u * u * tail.x +
          3 * u * u * t * c1.x +
          3 * u * t * t * c2.x +
          t * t * t * tip.x +
          nx * w,
        y:
          u * u * u * tail.y +
          3 * u * u * t * c1.y +
          3 * u * t * t * c2.y +
          t * t * t * tip.y +
          ny * w,
      });
    }
    if (arrowCrowdsCaption([...shaft, b1, b2], inflate, cap)) {
      bow *= 1.35;
    } else {
      geom = { shaft, c1 };
    }
  }
  if (!geom) {
    throw new Error(
      `${shot.file}: could not route the arrow clear of the caption ` +
        `(tip ${tip.x.toFixed(0)},${tip.y.toFixed(0)} — caption bbox too close to the target)`
    );
  }

  const f = (n) => n.toFixed(1);
  const d = geom.shaft.map((p, i) => `${i === 0 ? 'M' : 'L'}${f(p.x)} ${f(p.y)}`).join(' ');
  const svg = `<svg class="arrow-overlay" viewBox="0 0 ${FRAME_W} ${FRAME_H}" width="${FRAME_W}" height="${FRAME_H}">
      <path d="${d}"
        fill="none" stroke="${RED}" stroke-width="${f(stroke)}" stroke-linecap="round" stroke-linejoin="round"/>
      <path d="M${f(b1.x)} ${f(b1.y)} L${f(tip.x)} ${f(tip.y)} L${f(b2.x)} ${f(b2.y)}"
        fill="none" stroke="${RED}" stroke-width="${f(stroke)}" stroke-linecap="round" stroke-linejoin="round"/>
    </svg>`;
  return { svg, tail, bow, c1: geom.c1, c2, stroke };
}

// Scrawled red underline — the landing's hero .ul draw path.
function redUnderline({ width = 220 } = {}) {
  return `<svg class="ul" viewBox="0 0 120 14" style="width:${width}px;">
    <path d="M4 8 Q34 3 62 6 T116 6" fill="none" stroke="${RED}" stroke-width="4.2" stroke-linecap="round"/>
    <path d="M14 12 Q52 8 104 10" fill="none" stroke="${RED}" stroke-width="3.4" stroke-linecap="round"/>
  </svg>`;
}

const iconDataUri = (() => {
  const b64 = readFileSync(ICON_128).toString('base64');
  return `data:image/png;base64,${b64}`;
})();

// ---- PNG dimension reader (IHDR) — avoids a sharp dependency -----------------
function pngSize(buf) {
  // PNG signature is 8 bytes; IHDR length(4)+type(4) then width(4) height(4).
  if (buf.length < 24 || buf.readUInt32BE(0) !== 0x89504e47) {
    throw new Error('not a PNG');
  }
  return { width: buf.readUInt32BE(16), height: buf.readUInt32BE(20) };
}

// ---- STEP 1: capture the real popup in 5 states -----------------------------
// The last two (`fill`, `answers`) are the SAME connected #view-import as
// `connected`, forced into a deeper sub-state (autofill feedback / open answer
// tools with a drafted answer) via extra DOM manipulation — see the `sub`
// discriminator handled in the capture evaluate below. Only plain, serialisable
// data crosses into page.evaluate (functions don't serialise).
const STATES = [
  {
    name: 'offline',
    pillText: '✕ App not running',
    pillClass: 'pill pill--app_not_running',
    show: 'view-offline',
    // Arrow target: the "Get the app" button — the only VISIBLE control in the
    // forced offline view (#btn-retry lives in a hidden sibling and measures
    // 0x0, which the capture guard below rejects).
    target: '#btn-get-app',
  },
  {
    name: 'pairing',
    pillText: '⚠ Not paired',
    pillClass: 'pill pill--not_paired',
    show: 'view-pair',
    // Arrow target: the "Save & pair" button at the bottom of the pairing card.
    target: '#btn-save-token',
  },
  {
    name: 'connected',
    pillText: '● Connected',
    pillClass: 'pill pill--connected',
    show: 'view-import',
    // Arrow target: the "Import this job" primary button in the connected card.
    target: '#btn-import',
    importMsg: 'Imported "Senior Frontend Engineer". Open AI Job Hunter → Applications to view it.',
  },
  {
    name: 'fill',
    pillText: '● Connected',
    pillClass: 'pill pill--connected',
    show: 'view-import',
    // Arrow target: the "Fill this form" primary button in the Form group.
    target: '#btn-fill',
    // The REAL post-fill message string (popup.ts resolveFillResponse, the
    // non-name-split branch): `Filled N fields — review them on the page.`
    importMsg: 'Filled 4 fields — review them on the page.',
  },
  {
    name: 'answers',
    pillText: '● Connected',
    pillClass: 'pill pill--connected',
    show: 'view-import',
    // Arrow target: the AI-draft "Help me answer…" button inside answer tools.
    target: '#btn-assist',
    // The REAL streamed-draft-complete message (popup.ts: 'Draft ready — …').
    importMsg: 'Draft ready — review before using it.',
    // Deeper connected sub-state, forced in the capture evaluate below:
    //  - open the <details id="answer-tools"> disclosure,
    //  - hide the separate PR-11 "rewrite" sub-panel to keep the shot focused
    //    on drafting (both live in the same disclosure; not misrepresentation —
    //    a curated feature shot, not a claim the rewrite panel is absent),
    //  - fill the question box + reveal the drafted #assist-result.
    // The past-answer "Suggest answers" list is left in its default (unclicked)
    // empty state: the open card is already >2x the 800px frame, and the
    // drafted answer — which this shot's caption is about — is the hero. The
    // suggest feature still ships; it just isn't the subject of THIS shot.
    sub: 'answers',
    assistQuestion: 'Why do you want to work at Acme?',
    // Short, generic-professional draft (2 sentences) — plausible, not a real
    // user's data; drafted from the user's OWN résumé + page context, copy-only.
    assistDraft:
      "Acme's focus on shipping reliable tools that people depend on every day lines up with how I like to work. In my last role I owned the front-end of a similar product end to end, and I'd bring that same care for detail and users here.",
  },
];

async function capturePopups(browser) {
  // 320px popup body; render at 2x for crisp text, viewport at full CSS size
  // (the element screenshot is what we crop, so viewport just needs to fit it).
  const context = await browser.newContext({ deviceScaleFactor: DSF });
  // CRITICAL: block popup.js so its 3s timeout can't force the offline view.
  await context.route('**/popup.js', (route) => route.abort());

  const page = await context.newPage();
  await page.setViewportSize({ width: 360, height: 640 });

  const popupCss = readFileSync(DIST_CSS, 'utf8');

  const results = {};
  for (const state of STATES) {
    await page.goto(pathToFileURL(DIST_POPUP).href, { waitUntil: 'networkidle' });
    await page.addStyleTag({ content: popupCss });

    await page.evaluate((s) => {
      const pill = document.getElementById('status-pill');
      if (pill) {
        pill.textContent = ` ${s.pillText} `;
        pill.className = s.pillClass;
      }
      // Hide every view, then reveal only the target one. The popup CSS has
      // `[hidden]{display:none !important}` (popup.css), and `!important` beats
      // the `.view{display:flex}` rule, so `setAttribute('hidden')` DOES hide a
      // view and `removeAttribute('hidden')` restores the flex layout. The
      // explicit inline `display` writes below are belt-and-suspenders (they
      // keep the forced state robust even if that CSS rule is ever retuned).
      for (const sec of document.querySelectorAll('section.view')) {
        sec.setAttribute('hidden', '');
        sec.style.display = 'none';
      }
      const target = document.getElementById(s.show);
      if (target) {
        target.removeAttribute('hidden');
        target.style.display = 'flex';
      }

      if (s.importMsg) {
        const msg = document.getElementById('import-msg');
        if (msg) {
          msg.textContent = s.importMsg;
          msg.className = 'msg msg--ok';
        }
      }

      // Answer-tools sub-state (state `answers`): open the disclosure, hide the
      // separate PR-11 rewrite sub-panel, fill the question box, and reveal the
      // drafted answer. All source strings are plain serialisable data carried
      // on the state object.
      if (s.sub === 'answers') {
        const details = document.getElementById('answer-tools');
        if (details) details.open = true;

        // Hide the rewrite <div class="assist"> (owner of #rewrite-picker) so
        // the shot focuses on drafting, not the deferred rewrite feature.
        const rewritePicker = document.getElementById('rewrite-picker');
        const rewriteBlock = rewritePicker && rewritePicker.closest('.assist');
        if (rewriteBlock) rewriteBlock.setAttribute('hidden', '');

        const question = document.getElementById('assist-question');
        if (question && s.assistQuestion) question.value = s.assistQuestion;

        if (s.assistDraft) {
          const draft = document.getElementById('assist-draft');
          if (draft) draft.textContent = s.assistDraft;
          const result = document.getElementById('assist-result');
          if (result) result.removeAttribute('hidden');
        }
      }
    }, state);

    await page.evaluate(() => document.fonts.ready);

    const bg = await page.evaluate(() => {
      const bodyBg = getComputedStyle(document.body).backgroundColor;
      const appBg = getComputedStyle(document.querySelector('main.app')).backgroundColor;
      return { bodyBg, appBg };
    });
    // Popup body is now paper #f4ecdc (the "tasteful paper blend" restyle).
    const EXPECTED_BG = 'rgb(244, 236, 220)';
    if (bg.bodyBg !== EXPECTED_BG) {
      throw new Error(
        `UNSTYLED popup for state "${state.name}": body background is ${bg.bodyBg}, expected ${EXPECTED_BG} (#f4ecdc). popup.css did not apply — regenerate after \`pnpm -F @ajh/extension build:chrome\`.`
      );
    }
    console.log(`     popup-${state.name}: body bg ${bg.bodyBg}  (main.app bg ${bg.appBg})`);

    // Measure the target button RELATIVE TO `main.app`, as scale-independent
    // fractions of the captured card. The element screenshot below crops to
    // `main.app`, so these fractions stay valid no matter how the card PNG is
    // later scaled/placed in the 1280x800 frame — the frame-composition step
    // maps them into the placed card rect to anchor the arrow tip.
    const frac = await page.evaluate((sel) => {
      const app = document.querySelector('main.app');
      const target = document.querySelector(sel);
      if (!app || !target) return null;
      const appRect = app.getBoundingClientRect();
      const btnRect = target.getBoundingClientRect();
      return {
        fx: (btnRect.left - appRect.left) / appRect.width,
        fy: (btnRect.top - appRect.top) / appRect.height,
        fw: btnRect.width / appRect.width,
        fh: btnRect.height / appRect.height,
      };
    }, state.target);
    // Reject non-finite fractions explicitly: `NaN <= 0` is false, so a 0-width
    // main.app (division by zero → NaN/Infinity) would slip past the size check.
    if (
      !frac ||
      ![frac.fx, frac.fy, frac.fw, frac.fh].every(Number.isFinite) ||
      frac.fw <= 0 ||
      frac.fh <= 0
    ) {
      throw new Error(
        `Could not measure target "${state.target}" for state "${state.name}": ` +
          `element or main.app missing, hidden (zero size), or a non-finite ` +
          `measurement — the arrow would aim at garbage coordinates.`
      );
    }
    console.log(
      `     popup-${state.name}: ${state.target} fractions ` +
        `fx=${frac.fx.toFixed(4)} fy=${frac.fy.toFixed(4)} fw=${frac.fw.toFixed(4)} fh=${frac.fh.toFixed(4)}`
    );

    const card = page.locator('main.app');
    const buf = await card.screenshot({
      path: path.join(OUT_RAW, `popup-${state.name}.png`),
    });
    results[state.name] = { dim: pngSize(buf), frac };
  }

  await page.close();
  await context.close();
  return results;
}

// ---- STEP 2: composite doodle-annotated 1280x800 screenshots ----------------
// Each shot: paper bg + grain, popup PNG as a tilted card, scrawled caption,
// and a chunky hand-drawn arrow whose tip lands just OUTSIDE the card's LEFT
// edge, level with that state's primary control (Retry / Save & pair / Import
// this job / Fill this form / Help me answer…). The tip never crosses the card
// face: it targets the card LEFT EDGE x, at the button's vertical centre
// (capture-time fy/fh fractions).
//
// The card is `right:120px` × 560px wide (2px border) on a 1280px stage, so its
// rendered OUTER width is 564 and its visible LEFT edge sits at x = 596
// (FRAME_W − 120 − 564); it is vertically centred (or scrolled via cardAnchorY
// for the tall answers shot). The arrow is a full-frame SVG injected LAST
// (highest z-index) so the tip is never occluded by the popup image.
//
// Compositing is two-phase within one rendered page: render everything except
// the arrow, measure the caption's live bbox, then buildShotArrow() computes a
// per-shot curve from that bbox + the tip and injects it before the screenshot.
const FRAME_W = 1280;
const FRAME_H = 800;
const CARD_W = 560; // card image render width in frame px (matches .card img)
const CARD_RIGHT = 120; // .card right offset
// The .card has a 2px border, so its rendered OUTER width is CARD_W + 2*border
// and its visible LEFT edge sits CARD_OUTER_W left of (FRAME_W − CARD_RIGHT).
const CARD_BORDER = 2;
const CARD_OUTER_W = CARD_W + 2 * CARD_BORDER; // 564
const CARD_LEFT_EDGE = FRAME_W - CARD_RIGHT - CARD_OUTER_W; // 596, card's visible left edge
// Arrow tip sits just OUTSIDE the card's left edge (never crosses the card
// face). The tail is NOT fixed: it anchors per shot to the measured caption
// bbox at composite time — see buildShotArrow().
const TIP_GAP = 12; // tip sits this many px left of the card edge
const SHOTS = [
  {
    file: '01-private.png',
    raw: 'popup-offline.png',
    state: 'offline', // → measured fractions for #btn-retry
    caption: 'private by default — it only talks to the app on your own computer',
    captionPos: 'left:90px; top:150px; width:430px; text-align:left;',
    cardRotate: -1.4,
  },
  {
    file: '02-pair-once.png',
    raw: 'popup-pairing.png',
    state: 'pairing', // → measured fractions for #btn-save-token
    caption: 'pair once — paste the token from the desktop app',
    captionPos: 'left:90px; top:150px; width:430px; text-align:left;',
    cardRotate: 1.2,
  },
  {
    file: '03-one-click.png',
    raw: 'popup-connected.png',
    state: 'connected', // → measured fractions for #btn-import
    caption: "one click → the job's saved in your app, tagged New",
    captionPos: 'left:90px; top:150px; width:430px; text-align:left;',
    cardRotate: -1.0,
  },
  {
    file: '04-fill-forms.png',
    raw: 'popup-fill.png',
    state: 'fill', // → measured fractions for #btn-fill
    caption:
      'opt-in autofill — your saved details, filled on your command. you review every field; it never submits.',
    captionPos: 'left:90px; top:140px; width:440px; text-align:left;',
    cardRotate: 1.0,
  },
  {
    file: '05-answers.png',
    raw: 'popup-answers.png',
    state: 'answers', // → measured fractions for #btn-assist
    caption:
      "stuck on 'why do you want to work here?' — it drafts an answer from your resume. copy it when you're happy.",
    captionPos: 'left:90px; top:140px; width:440px; text-align:left;',
    cardRotate: -1.2,
    // The answers card is ~2x the 800px frame (open answer tools + draft), so
    // scroll it up to keep the question box, "Help me answer…" button and the
    // full drafted answer + its Copy button in frame — tuned against the
    // measured #btn-assist fraction and the draft-card height.
    cardAnchorY: 0.66,
  },
];

// Compute the placed card rect in frame px and the ARROW TIP for a shot.
// `raw` is the capture result for this state: { dim:{width,height}, frac }.
function shotCardGeometry(shot, raw) {
  // Card image rect in frame px. Width is fixed; height preserves the raw
  // popup PNG aspect ratio (height:auto).
  //
  // Vertical placement: `cardAnchorY` is the fraction of the card height that
  // aligns to the frame's vertical centre. Default 0.5 centres the card (the
  // original behaviour — shots 01–03 stay byte-identical). A larger fraction
  // scrolls a TALL card up so a lower region (e.g. the answer-draft) shows,
  // letting the frame's overflow:hidden crop the top — the same crop-to-fit the
  // centred shots already rely on, just anchored on a chosen region.
  const anchor = shot.cardAnchorY ?? 0.5;
  const imgH = (CARD_W * raw.dim.height) / raw.dim.width;
  const imgTop = FRAME_H / 2 - anchor * imgH;
  const imgLeft = CARD_LEFT_EDGE + CARD_BORDER;
  // The TIP targets the CARD's LEFT EDGE, level with the button's vertical
  // centre — never the card face. The AIM point is the button's actual centre
  // (fx/fw too): the arrow ARRIVES pointing at it, so the head reads as
  // pointing AT the button, not merely at the card edge.
  const btnCenterY = imgTop + (raw.frac.fy + raw.frac.fh / 2) * imgH;
  const btnCenterX = imgLeft + (raw.frac.fx + raw.frac.fw / 2) * CARD_W;
  // The card is rendered with a small `rotate(cardRotate)` whose CSS
  // transform-origin is the default 50% 50% of the CARD ELEMENT's border box —
  // i.e. the card's OWN centre, `imgTop + imgH / 2`. For centred shots that
  // coincides with FRAME_H / 2; for anchored shots (cardAnchorY) the card is
  // shifted, so its centre — and the rotation pivot — moves with it. Rotate
  // both points about that pivot to track the actually-rendered card.
  const cxc = CARD_LEFT_EDGE + CARD_OUTER_W / 2;
  const cyc = imgTop + imgH / 2;
  const theta = (shot.cardRotate * Math.PI) / 180;
  const cos = Math.cos(theta);
  const sin = Math.sin(theta);
  const rot = (px, py) => ({
    x: cxc + (px - cxc) * cos - (py - cyc) * sin,
    y: cyc + (px - cxc) * sin + (py - cyc) * cos,
  });
  const edge = rot(CARD_LEFT_EDGE, btnCenterY);
  const aim = rot(btnCenterX, btnCenterY);
  // Tip sits TIP_GAP px left of the (rotated) card edge — just outside the card
  // face, level with the target button. The tail is resolved later against the
  // live-measured caption bbox (buildShotArrow).
  const tip = { x: edge.x - TIP_GAP, y: edge.y };
  return { tip, aim, imgTop };
}

function shotFragment(shot, raw) {
  const rawBuf = readFileSync(path.join(OUT_RAW, shot.raw));
  const popupUri = `data:image/png;base64,${rawBuf.toString('base64')}`;
  const geometry = shotCardGeometry(shot, raw);
  const { imgTop } = geometry;
  // NOTE: the body has NO arrow yet — it is measured, computed, and injected
  // into the live page just before the screenshot (see compositeShots).
  // Default (no cardAnchorY): centre the card. An explicit anchor pins the
  // card's OUTER top to the computed imgTop (minus the 2px border), matching
  // shotCardGeometry's imgTop so the arrow still lands on the target button.
  const cardTop = shot.cardAnchorY == null ? '50%' : `${(imgTop - CARD_BORDER).toFixed(2)}px`;
  const cardTransform =
    shot.cardAnchorY == null
      ? `translateY(-50%) rotate(${shot.cardRotate}deg)`
      : `rotate(${shot.cardRotate}deg)`;
  const css = `
    .card{position:absolute; right:${CARD_RIGHT}px; top:${cardTop}; z-index:10;
      transform:${cardTransform};
      border:2px solid var(--ink); border-radius:14px;
      box-shadow:6px 10px 0 rgba(28,24,18,.14), 0 18px 40px rgba(28,24,18,.20);
      overflow:hidden; background:#0f1117;}
    .card img{display:block; width:${CARD_W}px; height:auto;}
    .caption{position:absolute; ${shot.captionPos} z-index:20;
      font-family:var(--scrawl); font-size:30px; line-height:1.42; color:var(--ink);
      transform:rotate(-2deg);}
    .caption .hl{color:var(--red);}
    /* Full-frame overlay injected LAST with the highest z-index, so the chunky
       arrow head sits ON TOP of the card edge and is never occluded. */
    .arrow-overlay{position:absolute; left:0; top:0; z-index:40; overflow:visible;
      pointer-events:none; filter:drop-shadow(2px 3px 0 rgba(28,24,18,.18));}`;
  const body = `
    <div class="grain"></div>
    <div class="caption">${shot.caption}</div>
    <div class="card"><img src="${popupUri}" alt=""></div>`;
  return { body, css, geometry };
}

// `postSetup(page)` (optional) runs after fonts settle and before the
// screenshot — the seam the shot compositor uses to measure the live page and
// inject the per-shot arrow.
async function renderSized(browser, bodyHtml, headExtra, css, target, outPath, postSetup) {
  // The layout is authored at FULL target dims (1280x800 etc.) for readability.
  // To get an EXACT target-pixel PNG at 2x crispness we render into a viewport
  // of HALF the target size with deviceScaleFactor:2, and scale the full-size
  // stage down by 1/DSF (transform:scale) so it fits the viewport. The 2x DSF
  // then renders that scaled stage back up -> output = (target/2)*2 = target,
  // pixel-exact, with text rasterised at 2x.
  const vw = target.w / DSF;
  const vh = target.h / DSF;
  const html = `<!doctype html><html><head><meta charset="utf-8">${FONTS_LINK}${headExtra || ''}
<style>
  :root{${FONT_VARS}}
  *{box-sizing:border-box; margin:0; padding:0;}
  html,body{width:${vw}px; height:${vh}px; overflow:hidden; background:var(--paper);}
  #stage{width:${target.w}px; height:${target.h}px; position:relative; overflow:hidden;
    transform:scale(${1 / DSF}); transform-origin:top left; background:var(--paper);
    font-family:var(--hand); color:var(--ink);}
  ${GRAIN_CSS}
  ${css}
</style></head>
<body><div id="stage">${bodyHtml}</div></body></html>`;

  const context = await browser.newContext({ deviceScaleFactor: DSF });
  const page = await context.newPage();
  await page.setViewportSize({ width: vw, height: vh });
  await page.setContent(html, { waitUntil: 'networkidle' });
  await page.evaluate(() => document.fonts.ready);
  if (postSetup) await postSetup(page);
  const buf = await page.screenshot({
    path: outPath,
    clip: { x: 0, y: 0, width: vw, height: vh },
  });
  await page.close();
  await context.close();
  return pngSize(buf);
}

async function compositeShots(browser, raw) {
  const dims = {};
  console.log('\n=== ARROW ANCHORING (per-shot dynamic geometry) ===');
  for (const shot of SHOTS) {
    const out = path.join(OUT_SHOTS, shot.file);
    const captured = raw[shot.state];
    const { body, css, geometry } = shotFragment(shot, captured);
    const { fx, fy, fw, fh } = captured.frac;
    const { tip, aim } = geometry;
    const targetSel = STATES.find((s) => s.name === shot.state).target;
    // Two-phase composite in ONE rendered page: everything except the arrow is
    // laid out, the caption's live bbox is measured, then the per-shot arrow
    // is computed + injected before the screenshot. getBoundingClientRect
    // reports viewport CSS px (the stage is scaled by 1/DSF with a top-left
    // origin), so multiply back up to frame px.
    const drawArrow = async (page) => {
      const r = await page.evaluate(() => {
        const box = document.querySelector('.caption').getBoundingClientRect();
        return { x: box.x, y: box.y, w: box.width, h: box.height };
      });
      const cap = { x: r.x * DSF, y: r.y * DSF, w: r.w * DSF, h: r.h * DSF };
      const arrow = buildShotArrow(shot, tip, aim, cap);
      await page.evaluate((svg) => {
        document.getElementById('stage').insertAdjacentHTML('beforeend', svg);
      }, arrow.svg);
      console.log(
        `     ${shot.file}  ${shot.state} → ${targetSel}\n` +
          `        fractions  fx=${fx.toFixed(4)} fy=${fy.toFixed(4)} fw=${fw.toFixed(4)} fh=${fh.toFixed(4)}\n` +
          `        caption bbox (${cap.x.toFixed(0)},${cap.y.toFixed(0)}) ${cap.w.toFixed(0)}x${cap.h.toFixed(0)}\n` +
          `        tail[${arrow.tail.mode}] (${arrow.tail.x.toFixed(1)}, ${arrow.tail.y.toFixed(1)})  ` +
          `c1 (${arrow.c1.x.toFixed(1)}, ${arrow.c1.y.toFixed(1)})  ` +
          `c2 (${arrow.c2.x.toFixed(1)}, ${arrow.c2.y.toFixed(1)})\n` +
          `        tip (${tip.x.toFixed(1)}, ${tip.y.toFixed(1)})  aim (${aim.x.toFixed(1)}, ${aim.y.toFixed(1)})  ` +
          `bow ${arrow.bow.toFixed(1)}  stroke ${arrow.stroke.toFixed(1)}`
      );
    };
    dims[shot.file] = await renderSized(
      browser,
      body,
      '',
      css,
      { w: 1280, h: 800 },
      out,
      drawArrow
    );
  }
  return dims;
}

// ---- STEP 3: promo tile + marquee -------------------------------------------
function promoTileFragment() {
  const css = `
    .center{position:absolute; inset:0; display:flex; flex-direction:column;
      align-items:center; justify-content:center;}
    .mark{width:88px; height:88px;
      filter:drop-shadow(3px 4px 0 rgba(28,24,18,.16)); margin-bottom:10px;}
    .title{font-family:var(--impact); font-size:40px; letter-spacing:.5px;
      line-height:1; color:var(--ink);}
    .val-wrap{position:relative; margin-top:14px;}
    .val{font-family:var(--scrawl); font-size:18px; color:var(--ink); transform:rotate(-1.5deg);}
    .ul{position:absolute; left:50%; bottom:-24px; transform:translateX(-50%); overflow:visible;}`;
  const body = `
    <div class="grain"></div>
    <div class="center">
      <img class="mark" src="${iconDataUri}" alt="">
      <div class="title">AI Job Hunter</div>
      <div class="val-wrap">
        <div class="val">One-click job import</div>
        ${redUnderline({ width: 190 })}
      </div>
    </div>`;
  return { body, css };
}

function marqueeFragment() {
  const css = `
    .row{position:absolute; inset:0; display:flex; align-items:center;
      gap:70px; padding:0 110px;}
    .mark{width:200px; height:200px; flex:none;
      filter:drop-shadow(5px 7px 0 rgba(28,24,18,.16)); transform:rotate(-3deg);}
    .copy{position:relative;}
    .headline{font-family:var(--impact); font-size:76px; line-height:1.02;
      letter-spacing:.5px; color:var(--ink); max-width:880px;}
    .headline .hl{color:var(--red);}
    .sub-wrap{position:relative; margin-top:26px; display:inline-block;}
    .sub{font-family:var(--scrawl); font-size:26px; line-height:1.5; color:var(--ink);
      transform:rotate(-1deg); max-width:760px;}
    .accent{position:absolute; right:120px; top:70px; transform:rotate(-6deg);
      filter:drop-shadow(2px 3px 0 rgba(28,24,18,.18));}
    .ul{position:absolute; left:0; bottom:-28px; overflow:visible;}`;
  const body = `
    <div class="grain"></div>
    <div class="accent">${redArrow({ rotate: 130, flip: false, width: 150 })}</div>
    <div class="row">
      <img class="mark" src="${iconDataUri}" alt="">
      <div class="copy">
        <div class="headline">Import any job with <span class="hl">one click.</span></div>
        <div class="sub-wrap">
          <div class="sub">Straight to your local desktop app.<br>No account. No cloud.</div>
          ${redUnderline({ width: 300 })}
        </div>
      </div>
    </div>`;
  return { body, css };
}

async function renderPromos(browser) {
  const dims = {};
  const tile = promoTileFragment();
  dims['promo-tile-440x280.png'] = await renderSized(
    browser,
    tile.body,
    '',
    tile.css,
    { w: 440, h: 280 },
    path.join(OUT_PROMO, 'promo-tile-440x280.png')
  );
  const marquee = marqueeFragment();
  dims['marquee-1400x560.png'] = await renderSized(
    browser,
    marquee.body,
    '',
    marquee.css,
    { w: 1400, h: 560 },
    path.join(OUT_PROMO, 'marquee-1400x560.png')
  );
  return dims;
}

// ---- main -------------------------------------------------------------------
async function main() {
  const browser = await chromium.launch();
  try {
    const raw = await capturePopups(browser);
    const shots = await compositeShots(browser, raw);
    const promos = await renderPromos(browser);

    const report = [];
    const expect = (file, dim, w, h) => {
      const ok = dim.width === w && dim.height === h;
      report.push(
        `${ok ? 'OK ' : 'BAD'}  ${file.padEnd(34)} ${dim.width}x${dim.height} (expect ${w}x${h})`
      );
      return ok;
    };

    console.log('\n=== RAW POPUPS (element screenshots, popup.js blocked) ===');
    for (const [name, r] of Object.entries(raw)) {
      console.log(`     screenshots/raw/popup-${name}.png  ${r.dim.width}x${r.dim.height}`);
    }
    console.log('\n=== STORE SCREENSHOTS (must be 1280x800) ===');
    let allOk = true;
    for (const [f, d] of Object.entries(shots))
      allOk = expect(`screenshots/${f}`, d, 1280, 800) && allOk;
    console.log('\n=== PROMO ===');
    allOk =
      expect('promo/promo-tile-440x280.png', promos['promo-tile-440x280.png'], 440, 280) && allOk;
    allOk =
      expect('promo/marquee-1400x560.png', promos['marquee-1400x560.png'], 1400, 560) && allOk;

    console.log('\n' + report.join('\n'));
    console.log(`\n${allOk ? 'ALL DIMENSIONS EXACT ✓' : 'DIMENSION MISMATCH ✗'}\n`);
    if (!allOk) process.exitCode = 1;
  } finally {
    await browser.close();
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
