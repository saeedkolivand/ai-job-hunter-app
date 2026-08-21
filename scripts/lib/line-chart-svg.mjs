// A hand-drawn "dark drafting paper" chart, as a standalone SVG string.
//
// Rendered once per day in CI and committed to the orphan `badges` branch, so
// the README embeds a static file instead of calling a live service.
//
// WHY IT LOOKS LIKE THIS: apps/landing is not a generic dark site, it is a
// notebook. Its devices, all reused here:
//   - a fractalNoise grain overlay              (home.css:48)
//   - ruled lines with a red margin rule        (creature.css body::before)
//   - HARD offset shadows, never a soft blur    (home.css `3px 4px 0`)
//   - Patrick Hand as the body face             (marketing-tokens.css --hand)
//   - stroke-dasharray draw-on animation        (home.css `.draw path`)
// Dark surface + text are the landing's own `body` (#0a0c11 / #e7ecf3).
//
// The line is drawn TWICE with deterministic per-point jitter, the way someone
// retraces a pencil stroke. Deterministic, not random: a fresh wobble every run
// would make every rebuild a visual diff for no reason.
//
// The annotation is the part no other star chart has. A growth curve tells you
// THAT something happened; this one says WHAT — the steepest day is found in the
// data and labelled in handwriting on a leader line.
//
// OPAQUE BACKGROUND ON PURPOSE: GitHub proxies README images through camo, where
// `prefers-color-scheme` inside an SVG is not reliably honoured, so a
// transparent chart would be at the mercy of whichever backdrop it landed on.
//
// FONT EMBEDDED ON PURPOSE: web fonts never load through camo. The woff2 is
// vendored (apps/landing/public/fonts, SIL OFL) and inlined as a data URI — the
// same technique star-history.com used. ~33KB per SVG, free here because the
// publish target is a parentless orphan commit that keeps no history.

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// Series accents, both chosen to carry against the dark surface below.
export const RED = '#e24b4a'; // --red (home/download/creature)
export const GOLD = '#f5b301'; // the README's existing stars-badge colour

// Surface + text. Keep in lockstep with `body` in apps/landing/src/styles/home.css.
const BG = '#0a0c11';
const FG = '#e7ecf3';

const FONT_PATH = join(
  dirname(fileURLToPath(import.meta.url)),
  '..',
  '..',
  'apps',
  'landing',
  'public',
  'fonts',
  'patrick-hand-400-latin.woff2'
);

let fontDataUri = null;

/** Base64 the vendored Patrick Hand once per process. */
function handFont() {
  if (fontDataUri === null) {
    fontDataUri = 'data:font/woff2;base64,' + readFileSync(FONT_PATH).toString('base64');
  }
  return fontDataUri;
}

/** Minimal XML escaping — these strings reach an SVG text node. */
function esc(s) {
  const map = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&apos;' };
  return String(s).replace(/[&<>"']/g, (c) => map[c]);
}

/** 1234 -> "1.2k". Mirrors the badge's humanize() so the two never disagree. */
function humanize(n) {
  return Math.abs(n) < 1000 ? String(n) : (n / 1000).toFixed(1) + 'k';
}

/** "Aug 20" from an ISO date, in UTC so a runner's timezone cannot shift it. */
function shortDate(iso) {
  return new Date(iso + 'T00:00:00Z').toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    timeZone: 'UTC',
  });
}

/**
 * Deterministic pseudo-random in [-1, 1] from an integer seed.
 *
 * A sine hash rather than Math.random: the wobble must be identical on every
 * rebuild, or each daily run would rewrite every coordinate and the chart would
 * shimmer between publishes for no reason.
 */
function jitter(seed) {
  const x = Math.sin(seed * 127.1) * 43758.5453;
  return (x - Math.floor(x)) * 2 - 1;
}

/**
 * An axis maximum at or above `max` that divides evenly into `rows` gridlines.
 *
 * Rounding the MAXIMUM to something round is not enough: 48 rounds to a tidy 50,
 * but 50 over 4 rows labels the axis 13 / 25 / 38. Rounding the STEP instead and
 * multiplying back up keeps every label whole (15 / 30 / 45 / 60).
 */
function axisMax(max, rows) {
  const raw = Math.max(max, 1) / rows;
  const pow = Math.pow(10, Math.floor(Math.log10(raw)));
  const mantissa = raw / pow;
  const nice = [1, 1.5, 2, 2.5, 3, 4, 5, 7.5, 10].find((c) => mantissa <= c) ?? 10;
  return nice * pow * rows;
}

/** The steepest single step in the series — the day worth pointing at. */
function peakStep(points) {
  let best = null;
  for (let i = 1; i < points.length; i++) {
    const gain = points[i].value - points[i - 1].value;
    if (gain > 0 && (!best || gain > best.gain)) best = { index: i, gain };
  }
  return best;
}

/**
 * @param {object} o
 * @param {{date:string,value:number}[]} o.points ascending by date, at least one
 * @param {string} o.title
 * @param {string} o.accent
 * @param {string} [o.subtitle]
 * @param {string} [o.noun] what one unit is, for the annotation ("stars")
 */
export function renderLineChart({ points, title, accent, subtitle = '', noun = '' }) {
  if (!points.length) throw new Error('renderLineChart: no points');

  const W = 860;
  const H = 360;
  const padL = 74;
  const padR = 30;
  const padT = 78;
  const padB = 46;
  const plotW = W - padL - padR;
  const plotH = H - padT - padB;
  const baseline = padT + plotH;

  const rows = 4;
  const maxV = axisMax(Math.max(...points.map((p) => p.value), 1), rows);
  // A single data point has no span to divide by — pin it to the right edge so
  // the chart reads as "one reading so far" rather than dividing by zero.
  const xOf = (i) =>
    points.length === 1 ? padL + plotW : padL + (i / (points.length - 1)) * plotW;
  const yOf = (v) => padT + plotH - (v / maxV) * plotH;

  /** The series as a jittered polyline; `pass` shifts the wobble for the retrace. */
  const stroke = (pass) =>
    points
      .map((p, i) => {
        const wob = points.length > 60 ? 0.55 : 1.15; // a dense series needs a calmer hand
        const dx = jitter(i * 2 + pass * 91) * wob;
        const dy = jitter(i * 2 + 1 + pass * 91) * wob;
        return `${(xOf(i) + dx).toFixed(1)},${(yOf(p.value) + dy).toFixed(1)}`;
      })
      .join(' ');

  const line = stroke(0);
  const area = `${padL},${baseline} ${line} ${xOf(points.length - 1).toFixed(1)},${baseline}`;

  // Approximate path length for the draw-on dasharray. Over-estimating is safe
  // (the line just finishes early); under-estimating would leave a visible gap.
  let pathLen = 0;
  for (let i = 1; i < points.length; i++) {
    pathLen += Math.hypot(xOf(i) - xOf(i - 1), yOf(points[i].value) - yOf(points[i - 1].value));
  }
  pathLen = Math.ceil(pathLen * 1.1) + 10;

  // Ruled notebook lines: denser than the value gridlines and much fainter, so
  // the page reads as paper rather than as a chart with too many gridlines.
  const rules = [];
  for (let y = padT - 14; y < baseline + 18; y += 22) {
    rules.push(
      `<line x1="${padL - 26}" y1="${y.toFixed(1)}" x2="${W - 22}" y2="${y.toFixed(1)}" stroke="${FG}" stroke-opacity="0.05" stroke-width="1"/>`
    );
  }

  const grid = Array.from({ length: rows + 1 }, (_, i) => {
    const v = (maxV / rows) * i;
    const y = yOf(v).toFixed(1);
    return (
      `<line x1="${padL}" y1="${y}" x2="${padL + plotW}" y2="${y}" stroke="${FG}" ` +
      `stroke-opacity="0.14" stroke-width="1" stroke-dasharray="3 6"/>\n    ` +
      `<text x="${padL - 12}" y="${(Number(y) + 5).toFixed(1)}" fill="${FG}" ` +
      `fill-opacity="0.55" font-size="14" text-anchor="end">${humanize(Math.round(v))}</text>`
    );
  }).join('\n    ');

  // First / middle / last only — more labels than that collide at this width.
  const idx =
    points.length <= 2 ? [0, points.length - 1] : [0, (points.length - 1) >> 1, points.length - 1];
  const ticks = [...new Set(idx)]
    .map((i) => {
      const anchor = i === 0 ? 'start' : i === points.length - 1 ? 'end' : 'middle';
      return (
        `<text x="${xOf(i).toFixed(1)}" y="${baseline + 26}" fill="${FG}" fill-opacity="0.55" ` +
        `font-size="14" text-anchor="${anchor}">${esc(shortDate(points[i].date))}</text>`
      );
    })
    .join('\n    ');

  // The annotation. Skipped on a flat or very short series — pointing at nothing
  // with a handwritten note would be worse than saying nothing.
  let note = '';
  const peak = peakStep(points);
  if (peak && points.length > 3) {
    const p = points[peak.index];
    const px = xOf(peak.index);
    const py = yOf(p.value);
    // Keep the label inside the card: flip it to the left once the point sits in
    // the right third, where a right-hand label would overflow the edge.
    const flip = px > padL + plotW * 0.62;
    const lx = flip ? px - 18 : px + 18;
    const ly = Math.max(py - 34, padT + 14);
    const label = `+${peak.gain} ${noun}`.trim();
    const anchor = flip ? 'end' : 'start';
    note =
      `<line x1="${px.toFixed(1)}" y1="${py.toFixed(1)}" x2="${lx.toFixed(1)}" y2="${(ly + 6).toFixed(1)}" ` +
      `stroke="${FG}" stroke-opacity="0.4" stroke-width="1.5" stroke-dasharray="2 3"/>\n      ` +
      `<circle cx="${px.toFixed(1)}" cy="${py.toFixed(1)}" r="3.5" fill="none" stroke="${FG}" stroke-opacity="0.75" stroke-width="1.5"/>\n      ` +
      `<text x="${lx.toFixed(1)}" y="${ly.toFixed(1)}" fill="${FG}" fill-opacity="0.9" font-size="15" text-anchor="${anchor}">${esc(label)}</text>\n      ` +
      `<text x="${lx.toFixed(1)}" y="${(ly + 16).toFixed(1)}" fill="${FG}" fill-opacity="0.5" font-size="12.5" text-anchor="${anchor}">${esc(shortDate(p.date))}</text>`;
  }

  const latest = points[points.length - 1];
  const growth = latest.value - points[0].value;
  const delta = `${growth >= 0 ? '+' : ''}${humanize(growth)} since ${shortDate(points[0].date)}`;

  return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" role="img" aria-label="${esc(title)}: ${latest.value} as of ${esc(latest.date)}">
  <title>${esc(title)} — ${latest.value} as of ${esc(latest.date)}</title>
  <desc>${esc(subtitle)}</desc>
  <defs>
    <style>
      @font-face{font-family:"Patrick Hand";font-style:normal;font-weight:400;src:url(${handFont()}) format("woff2");}
      .ink{font-family:'Patrick Hand',cursive}
      /* The finished state is the DEFAULT state, and the animation only plays
         back INTO it (from-keyframes + backwards fill). An earlier version set
         the hidden state statically and animated out of it, which drew a chart
         with no data line anywhere the animation did not run — and it does not
         run in every img-embedded-SVG context. Never let a stylesheet be the
         only thing standing between a reader and the data. */
      .draw{stroke-dasharray:${pathLen};stroke-dashoffset:0;animation:draw 1.8s ease-out backwards}
      .fade{opacity:1;animation:fade .7s ease-out 1.5s backwards}
      @keyframes draw{from{stroke-dashoffset:${pathLen}}}
      @keyframes fade{from{opacity:0}}
      @media (prefers-reduced-motion:reduce){
        .draw,.fade{animation:none}
      }
    </style>
    <filter id="grain"><feTurbulence type="fractalNoise" baseFrequency="0.9" numOctaves="2"/></filter>
    <linearGradient id="fill" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="${accent}" stop-opacity="0.34"/>
      <stop offset="100%" stop-color="${accent}" stop-opacity="0.02"/>
    </linearGradient>
  </defs>
  <rect x="6" y="7" width="${W - 10}" height="${H - 10}" rx="12" fill="#000" opacity="0.55"/>
  <rect x="0" y="0" width="${W - 10}" height="${H - 10}" rx="12" fill="${BG}"/>
  <rect x="0" y="0" width="${W - 10}" height="${H - 10}" rx="12" filter="url(#grain)" opacity="0.055"/>
  <rect x="1" y="1" width="${W - 12}" height="${H - 12}" rx="11" fill="none" stroke="${FG}" stroke-opacity="0.22" stroke-width="2"/>
  <g class="ink">
    ${rules.join('\n    ')}
    <line x1="${padL - 26}" y1="8" x2="${padL - 26}" y2="${H - 18}" stroke="${RED}" stroke-opacity="0.45" stroke-width="1.5"/>
    <text x="${padL - 10}" y="38" fill="${FG}" font-size="23">${esc(title)}</text>
    <text x="${padL - 10}" y="58" fill="${FG}" fill-opacity="0.6" font-size="13.5">${esc(subtitle)}</text>
    <text x="${W - padR}" y="40" fill="${accent}" font-size="34" text-anchor="end">${humanize(latest.value)}</text>
    <text x="${W - padR}" y="59" fill="${FG}" fill-opacity="0.6" font-size="13.5" text-anchor="end">${esc(delta)}</text>
    ${grid}
    <polygon class="fade" points="${area}" fill="url(#fill)"/>
    <polyline class="draw" points="${stroke(1)}" fill="none" stroke="${accent}" stroke-opacity="0.45" stroke-width="4.5" stroke-linejoin="round" stroke-linecap="round"/>
    <polyline class="draw" points="${line}" fill="none" stroke="${accent}" stroke-width="2.6" stroke-linejoin="round" stroke-linecap="round"/>
    <circle class="fade" cx="${xOf(points.length - 1).toFixed(1)}" cy="${yOf(latest.value).toFixed(1)}" r="5" fill="${accent}" stroke="${BG}" stroke-width="2.5"/>
    <g class="fade">
      ${note}
    </g>
    ${ticks}
  </g>
</svg>
`;
}
