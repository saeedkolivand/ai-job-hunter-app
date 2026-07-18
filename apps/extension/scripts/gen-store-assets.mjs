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
// 5-screenshot set — those use frameArrowSvg(), anchored to measured buttons).
function redArrow({ rotate = 0, flip = false, width = 150 } = {}) {
  const sx = flip ? -1 : 1;
  return `<svg class="arrow" viewBox="0 0 50 44" style="width:${width}px;transform:rotate(${rotate}deg) scaleX(${sx});">
    <path d="M46 10 Q22 10 8 32" fill="none" stroke="${RED}" stroke-width="3.6" stroke-linecap="round"/>
    <path d="M8 32 l13 -2 M8 32 l4 -13" fill="none" stroke="${RED}" stroke-width="3.6" stroke-linecap="round"/>
  </svg>`;
}

// Hand-drawn red pointer arrow for the 5 store screenshots, built directly in
// FRAME coordinates as a full-frame SVG overlay. Tail and tip are absolute
// (x,y) points in the 1280x800 stage; the path is a single quadratic curve
// from tail→tip with a clear two-barb arrowhead at the tip.
//
// Same chunky stroke / curve STYLE in all shots — in fact the SAME shape,
// translated vertically only. The tip always lands just OUTSIDE the card's LEFT
// edge (constant x across shots), level with the target button's vertical centre
// (mapped from the capture-time fy/fh fractions). It never crosses the card
// face — the head points AT the card from outside, regardless of which button
// (Retry / Save & pair / Import this job / Fill this form / Help me answer…)
// each shot highlights.
//
// Rendered as the LAST element in the body (after the card) with the highest
// z-index, so the head is never occluded by the popup image.
const ARROW = {
  stroke: 5.5, // chunky hand-drawn weight
  barb: 26, // arrowhead barb length in frame px
};

// Build a full-frame SVG overlay with a curved arrow from `tail` to `tip`.
// The control point bows the shaft so it reads as a relaxed hand-drawn sweep
// (perpendicular offset from the tail→tip midpoint). The arrowhead is two
// barbs rotated to the incoming direction at the tip.
function frameArrowSvg(tail, tip, frame) {
  const dx = tip.x - tail.x;
  const dy = tip.y - tail.y;
  const len = Math.hypot(dx, dy) || 1;
  // Unit direction tail→tip and its left-hand normal.
  const ux = dx / len;
  const uy = dy / len;
  const nx = -uy;
  const ny = ux;
  // Bow the curve "upward" (toward the caption) by ~14% of its length.
  const mx = (tail.x + tip.x) / 2;
  const my = (tail.y + tip.y) / 2;
  const bow = -0.14 * len;
  const cx = mx + nx * bow;
  const cy = my + ny * bow;
  // Arrowhead: two barbs swept back from the tip along the incoming direction,
  // splayed ±28° so the head reads clearly.
  const ang = Math.atan2(tip.y - cy, tip.x - cx); // incoming direction at tip
  const spread = (28 * Math.PI) / 180;
  const b = ARROW.barb;
  const b1x = tip.x - b * Math.cos(ang - spread);
  const b1y = tip.y - b * Math.sin(ang - spread);
  const b2x = tip.x - b * Math.cos(ang + spread);
  const b2y = tip.y - b * Math.sin(ang + spread);
  const f = (n) => n.toFixed(1);
  return `<svg class="arrow-overlay" viewBox="0 0 ${frame.w} ${frame.h}" width="${frame.w}" height="${frame.h}">
      <path d="M${f(tail.x)} ${f(tail.y)} Q${f(cx)} ${f(cy)} ${f(tip.x)} ${f(tip.y)}"
        fill="none" stroke="${RED}" stroke-width="${ARROW.stroke}" stroke-linecap="round" stroke-linejoin="round"/>
      <path d="M${f(b1x)} ${f(b1y)} L${f(tip.x)} ${f(tip.y)} L${f(b2x)} ${f(b2y)}"
        fill="none" stroke="${RED}" stroke-width="${ARROW.stroke}" stroke-linecap="round" stroke-linejoin="round"/>
    </svg>`;
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
    // Arrow target: the "Retry" button in the offline card.
    target: '#btn-retry',
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
    if (!frac) {
      throw new Error(
        `Could not measure target "${state.target}" for state "${state.name}": element or main.app missing.`
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
// face: it targets the constant card LEFT EDGE x, and only the button's
// vertical centre (capture-time fy/fh) moves it up/down — so the arrow is one
// identical shape translated vertically.
//
// The card is `right:120px` × 560px wide (2px border) on a 1280px stage, so its
// rendered OUTER width is 564 and its visible LEFT edge sits at x = 596
// (FRAME_W − 120 − 564); it is vertically centred (or scrolled via cardAnchorY
// for the tall answers shot). The arrow is a full-frame SVG drawn LAST (highest
// z-index) so the tip is never occluded by the popup image.
//
// The tail is a fixed up-left offset from the tip (TAIL_DX/TAIL_DY), so it too
// translates vertically with the tip and the whole arrow keeps one shape.
const FRAME_W = 1280;
const FRAME_H = 800;
const CARD_W = 560; // card image render width in frame px (matches .card img)
const CARD_RIGHT = 120; // .card right offset
// The .card has a 2px border, so its rendered OUTER width is CARD_W + 2*border
// and its visible LEFT edge sits CARD_OUTER_W left of (FRAME_W − CARD_RIGHT).
const CARD_BORDER = 2;
const CARD_OUTER_W = CARD_W + 2 * CARD_BORDER; // 564
const CARD_LEFT_EDGE = FRAME_W - CARD_RIGHT - CARD_OUTER_W; // 596, card's visible left edge
const IMG_LEFT = CARD_LEFT_EDGE + CARD_BORDER; // 598, where the popup PNG actually sits
// Arrow tip sits just OUTSIDE the card's left edge (never crosses the card
// face); the tail is a fixed up-left offset from the tip. Because CARD_LEFT_EDGE
// is constant across all shots (card width is fixed), the arrow is one identical
// shape translated vertically only — y tracks each target button's centre.
const TIP_GAP = 12; // tip sits this many px left of the card edge
const TAIL_DX = -210; // fixed up-left offset from tip (tunable)
const TAIL_DY = -118;
const SHOTS = [
  {
    file: '01-private.png',
    raw: 'popup-offline.png',
    state: 'offline', // → measured fractions for #btn-retry
    caption: 'private by default — it only talks to the app on your own computer',
    captionPos: 'left:90px; top:150px; width:430px; text-align:left;',
    // Caption top (px) — drives the tail's vertical anchor near the copy.
    captionTop: 150,
    cardRotate: -1.4,
  },
  {
    file: '02-pair-once.png',
    raw: 'popup-pairing.png',
    state: 'pairing', // → measured fractions for #btn-save-token
    caption: 'pair once — paste the token from the desktop app',
    captionPos: 'left:90px; top:150px; width:430px; text-align:left;',
    captionTop: 150,
    cardRotate: 1.2,
  },
  {
    file: '03-one-click.png',
    raw: 'popup-connected.png',
    state: 'connected', // → measured fractions for #btn-import
    caption: "one click → the job's saved in your app, tagged New",
    captionPos: 'left:90px; top:150px; width:430px; text-align:left;',
    captionTop: 150,
    cardRotate: -1.0,
  },
  {
    file: '04-fill-forms.png',
    raw: 'popup-fill.png',
    state: 'fill', // → measured fractions for #btn-fill
    caption:
      'opt-in autofill — your saved details, filled on your command. you review every field; it never submits.',
    captionPos: 'left:90px; top:140px; width:440px; text-align:left;',
    captionTop: 140,
    cardRotate: 1.0,
  },
  {
    file: '05-answers.png',
    raw: 'popup-answers.png',
    state: 'answers', // → measured fractions for #btn-assist
    caption:
      "stuck on 'why do you want to work here?' — it drafts an answer from your resume. copy it when you're happy.",
    captionPos: 'left:90px; top:140px; width:440px; text-align:left;',
    captionTop: 140,
    cardRotate: -1.2,
    // The answers card is ~2x the 800px frame (open answer tools + draft), so
    // scroll it up to keep the question box, "Help me answer…" button and the
    // full drafted answer + its Copy button in frame — tuned against the
    // measured #btn-assist fraction and the draft-card height.
    cardAnchorY: 0.66,
  },
];

// Compute the placed card rect in frame px and the arrow tip/tail for a shot.
// `raw` is the capture result for this state: { dim:{width,height}, frac }.
function shotArrowGeometry(shot, raw) {
  // Card image rect in frame px. Width is fixed; height preserves the raw
  // popup PNG aspect ratio (height:auto). The PNG sits at IMG_LEFT (inside the
  // 2px card border).
  //
  // Vertical placement: `cardAnchorY` is the fraction of the card height that
  // aligns to the frame's vertical centre. Default 0.5 centres the card (the
  // original behaviour — shots 01–03 stay byte-identical). A larger fraction
  // scrolls a TALL card up so a lower region (e.g. the answer-draft) shows,
  // letting the frame's overflow:hidden crop the top — the same crop-to-fit the
  // centred shots already rely on, just anchored on a chosen region.
  const anchor = shot.cardAnchorY ?? 0.5;
  const imgW = CARD_W;
  const imgH = (CARD_W * raw.dim.height) / raw.dim.width;
  const imgTop = FRAME_H / 2 - anchor * imgH;
  const imgLeft = IMG_LEFT;
  // Target the CARD's LEFT EDGE (constant across shots), level with the button's
  // vertical centre — NOT the button's left edge (fx), which is interior for the
  // centred Retry and full-width Save & pair buttons. Only fy/fh drive the tip
  // (its vertical position); fx/fw are ignored so the tip never lands inside the
  // card face.
  const btnCenterY = imgTop + (raw.frac.fy + raw.frac.fh / 2) * imgH;
  // Edge point in the UNROTATED frame: card left edge, at the button centre Y.
  let ex = CARD_LEFT_EDGE;
  let ey = btnCenterY;
  // The card is rendered with a small `rotate(cardRotate)` about its CENTRE, so
  // rotate the edge point by the same angle about that centre to track the
  // actually-rendered card edge.
  const cxc = CARD_LEFT_EDGE + CARD_OUTER_W / 2;
  const cyc = FRAME_H / 2;
  const theta = (shot.cardRotate * Math.PI) / 180;
  const cos = Math.cos(theta);
  const sin = Math.sin(theta);
  const rx = ex - cxc;
  const ry = ey - cyc;
  const rotatedEx = cxc + rx * cos - ry * sin;
  const rotatedEy = cyc + rx * sin + ry * cos;
  // Tip sits TIP_GAP px left of the (rotated) card edge — just outside the card
  // face. tip.x is ~constant across shots (rotation jitter only); tip.y tracks
  // each button → one identical arrow translated vertically.
  const tip = { x: rotatedEx - TIP_GAP, y: rotatedEy };
  // Tail is a fixed up-left offset from the tip.
  const tail = { x: tip.x + TAIL_DX, y: tip.y + TAIL_DY };
  return { tip, tail, imgLeft, imgTop, imgW, imgH };
}

function shotFragment(shot, raw) {
  const rawBuf = readFileSync(path.join(OUT_RAW, shot.raw));
  const popupUri = `data:image/png;base64,${rawBuf.toString('base64')}`;
  const geometry = shotArrowGeometry(shot, raw);
  const { tip, tail, imgTop } = geometry;
  const arrowSvg = frameArrowSvg(tail, tip, { w: FRAME_W, h: FRAME_H });
  // Default (no cardAnchorY): centre the card — shots 01–03 stay byte-identical.
  // An explicit anchor pins the card's OUTER top to the computed imgTop (minus
  // the 2px border), matching shotArrowGeometry's imgTop so the arrow still
  // lands on the target button.
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
    /* Full-frame overlay drawn LAST with the highest z-index, so the chunky
       arrow head sits ON TOP of the card edge and is never occluded. */
    .arrow-overlay{position:absolute; left:0; top:0; z-index:40; overflow:visible;
      pointer-events:none; filter:drop-shadow(2px 3px 0 rgba(28,24,18,.18));}`;
  const body = `
    <div class="grain"></div>
    <div class="caption">${shot.caption}</div>
    <div class="card"><img src="${popupUri}" alt=""></div>
    ${arrowSvg}`;
  return { body, css, geometry };
}

async function renderSized(browser, bodyHtml, headExtra, css, target, outPath) {
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
  console.log('\n=== ARROW ANCHORING (measured button fractions → frame tip) ===');
  for (const shot of SHOTS) {
    const out = path.join(OUT_SHOTS, shot.file);
    const captured = raw[shot.state];
    const { body, css, geometry } = shotFragment(shot, captured);
    const { fx, fy, fw, fh } = captured.frac;
    const { tip, tail } = geometry;
    const targetSel = STATES.find((s) => s.name === shot.state).target;
    console.log(
      `     ${shot.file}  ${shot.state} → ${targetSel}\n` +
        `        fractions  fx=${fx.toFixed(4)} fy=${fy.toFixed(4)} fw=${fw.toFixed(4)} fh=${fh.toFixed(4)}\n` +
        `        frame tip  (${tip.x.toFixed(1)}, ${tip.y.toFixed(1)})   tail (${tail.x.toFixed(1)}, ${tail.y.toFixed(1)})`
    );
    dims[shot.file] = await renderSized(browser, body, '', css, { w: 1280, h: 800 }, out);
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
