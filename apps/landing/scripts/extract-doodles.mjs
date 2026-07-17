// extract-doodles.mjs
// Parses the parity source-of-truth landing page (repo-root landing/index.html),
// finds every inline hand-drawn SVG (doods, deco scribbles, journey tips, the .ul
// underline, dc-arrow, crayon-arrow, stonks, inklines) and samples each path into
// flat polylines via svg-path-properties. Emits src/data/doodles.json for the GL
// ink engine to redraw as Line2 fat strokes. AST/text-only, no browser, no API.
//
// Run: pnpm --filter @ajh/landing extract:doodles
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { svgPathProperties } from 'svg-path-properties';

const HTML = new URL('../../../landing/index.html', import.meta.url);
const OUT = new URL('../src/data/doodles.json', import.meta.url);
const STEP = 2; // arc-length sample spacing, viewBox units
const r2 = (n) => Math.round(n * 100) / 100;

const SECTIONS = ['hero', 'beat1', 'beat2', 'beat3', 'beat4', 'features', 'testi', 'finale'];
// category -> default stroke width, mirroring the page CSS
const DEF_W = { dood: 3.2, deco: 3, inkline: 3, underline: 4.2, journey: 3, misc: 3 };

const html = readFileSync(HTML, 'utf8');
const body = html.slice(html.indexOf('<body'));

const failures = [];

// ---- attribute helpers ---------------------------------------------------
function attrs(tag) {
  const m = {};
  const re = /([a-zA-Z][\w-]*)\s*=\s*"([^"]*)"/g;
  let x;
  while ((x = re.exec(tag))) m[x[1]] = x[2];
  return m;
}
function styleProps(a) {
  const p = {};
  if (a.style) for (const part of a.style.split(';')) {
    const i = part.indexOf(':');
    if (i > 0) p[part.slice(0, i).trim()] = part.slice(i + 1).trim();
  }
  return p;
}

// ---- shape -> path d string ---------------------------------------------
function shapeToD(name, a) {
  switch (name) {
    case 'path': return a.d || '';
    case 'line': return `M${+a.x1},${+a.y1} L${+a.x2},${+a.y2}`;
    case 'polyline':
    case 'polygon': {
      const pts = (a.points || '').trim().split(/\s+/).map((p) => p.split(',').join(','));
      if (!pts[0]) return '';
      return `M${pts.join(' L')}` + (name === 'polygon' ? ' Z' : '');
    }
    case 'circle': {
      const cx = +a.cx, cy = +a.cy, rr = +a.r;
      return `M${cx - rr},${cy} a${rr},${rr} 0 1,0 ${2 * rr},0 a${rr},${rr} 0 1,0 ${-2 * rr},0 Z`;
    }
    case 'ellipse': {
      const cx = +a.cx, cy = +a.cy, rx = +a.rx, ry = +a.ry;
      return `M${cx - rx},${cy} a${rx},${ry} 0 1,0 ${2 * rx},0 a${rx},${ry} 0 1,0 ${-2 * rx},0 Z`;
    }
    case 'rect': {
      const x = +a.x, y = +a.y, w = +a.width, h = +a.height;
      return `M${x},${y} h${w} v${h} h${-w} Z`;
    }
    default: return '';
  }
}

function sample(d) {
  const props = new svgPathProperties(d);
  const len = props.getTotalLength();
  if (!(len > 0.5)) return null; // drop degenerate paths (M0 0, h.1 dot markers)
  const n = Math.max(2, Math.round(len / STEP));
  const pts = [];
  for (let i = 0; i <= n; i++) {
    const pt = props.getPointAtLength((i / n) * len);
    pts.push(r2(pt.x), r2(pt.y));
  }
  return pts;
}

// parse one shape element into a stroke record (or null to skip)
function makeStroke(tagName, tag, cls, defW, label) {
  const a = attrs(tag);
  const sp = styleProps(a);
  const d = shapeToD(tagName, a);
  if (!d) return null;
  let pts;
  try {
    pts = sample(d);
  } catch (e) {
    failures.push(`${label}: <${tagName}> ${e.message}`);
    return null;
  }
  if (!pts) return null;

  const fillStyle = sp.fill || a.fill;
  const strokeStyle = sp.stroke || a.stroke;
  const isFill = /\bdood-fill\b/.test(cls) ||
    (fillStyle && fillStyle !== 'none' && (!strokeStyle || strokeStyle === 'none'));
  if (isFill) {
    const rec = { fill: true, pts };
    if (fillStyle && fillStyle !== 'none') rec.color = fillStyle;
    return rec;
  }
  const rec = { pts, w: +(sp['stroke-width'] || a['stroke-width'] || defW) };
  if (strokeStyle && strokeStyle !== 'none') rec.color = strokeStyle;
  if (/\bdashed\b/.test(cls) || sp['stroke-dasharray'] || a['stroke-dasharray']) rec.dashed = true;
  return rec;
}

const SHAPE_RE = /<(path|circle|ellipse|rect|line|polyline|polygon)\b([^>]*)>/g;

// collect stroke records for a chunk of svg inner html
function collectStrokes(inner, defW, label) {
  const out = [];
  let m;
  SHAPE_RE.lastIndex = 0;
  while ((m = SHAPE_RE.exec(inner))) {
    const tag = m[0];
    const cls = (tag.match(/class="([^"]*)"/) || [undefined, ''])[1];
    const s = makeStroke(m[1], tag, cls, defW, label);
    if (s) out.push(s);
  }
  return out;
}

function viewBoxOf(svgTag, pts) {
  const vb = (svgTag.match(/viewBox="([^"]*)"/) || [undefined, ''])[1].trim().split(/\s+/).map(Number);
  if (vb.length === 4) return [vb[2], vb[3]];
  // no viewBox (journey): derive a box from sampled extent
  let mx = 0, my = 0;
  for (let i = 0; i < pts.length; i += 2) { mx = Math.max(mx, Math.abs(pts[i])); my = Math.max(my, Math.abs(pts[i + 1])); }
  return [r2(mx * 2), r2(my * 2)];
}

// ---- locate structural anchors ------------------------------------------
const sectionAt = [];
{
  const re = /<section class="([^"]*)"/g;
  let m;
  while ((m = re.exec(body))) {
    const name = SECTIONS.find((s) => m[1].split(/\s+/).includes(s));
    if (name) sectionAt.push({ i: m.index, name });
  }
}
const doodAt = [];
{
  const re = /<div class="doodle\s+([a-z0-9-]+)/g;
  let m;
  while ((m = re.exec(body))) doodAt.push({ i: m.index, name: m[1] });
}
const journeyStart = body.indexOf('<div id="journey"');

// ---- walk every inline <svg> in body order ------------------------------
const doodles = [];
const decoCount = {};
let inklineN = 0;
let dPtr = 0, activeDood = null, doodUsed = true;
let svgCount = 0;

const svgRe = /<svg\b[^>]*>[\s\S]*?<\/svg>/g;
let sm;
while ((sm = svgRe.exec(body))) {
  const block = sm[0];
  const at = sm.index;
  svgCount++;
  const openTag = block.slice(0, block.indexOf('>') + 1);
  const inner = block.slice(openTag.length, -6); // strip <svg ...> and </svg>
  const cls = (openTag.match(/class="([^"]*)"/) || [undefined, ''])[1];

  // advance doodle context
  while (dPtr < doodAt.length && doodAt[dPtr].i < at) {
    activeDood = doodAt[dPtr].name;
    doodUsed = false;
    dPtr++;
  }
  const section = (sectionAt.filter((s) => s.i < at).pop() || {}).name;

  // journey svg: two arrowhead tips, skip the empty journey-path placeholders
  if (journeyStart >= 0 && at >= journeyStart && block.includes('id="journey-path"')) {
    for (const tip of ['journey-tip', 'journey-tip-2']) {
      const g = inner.match(new RegExp(`<g id="${tip}">([\\s\\S]*?)</g>`));
      if (!g) continue;
      const strokes = collectStrokes(g[1], DEF_W.journey, tip);
      if (strokes.length) doodles.push({ name: tip, viewBox: viewBoxOf(openTag, strokes[0].pts), strokes });
    }
    continue;
  }

  let name, defW;
  if (/\bdeco\b/.test(cls)) {
    decoCount[section] = (decoCount[section] || 0) + 1;
    name = `deco-${section}-${decoCount[section]}`;
    defW = DEF_W.deco;
  } else if (/\binkline\b/.test(cls)) {
    name = `inkline-${++inklineN}`;
    defW = DEF_W.inkline;
  } else if (/\bdc-arrow\b/.test(cls)) {
    name = 'dc-arrow'; defW = DEF_W.misc;
  } else if (/\bcrayon-arrow\b/.test(cls)) {
    name = 'crayon-arrow'; defW = DEF_W.misc;
  } else if (/\bstonks\b/.test(cls)) {
    name = 'stonks'; defW = DEF_W.misc;
  } else if (/\bdraw\b/.test(cls)) {
    name = 'hero-underline'; defW = DEF_W.underline; // the only bare .draw svg (the .ul underline)
  } else if (activeDood && !doodUsed) {
    name = activeDood; defW = DEF_W.dood; doodUsed = true; // the poke-doodle figure
  } else {
    continue; // functional/icon svgs (sound-toggle, ats robot, npcs) are out of scope
  }

  const strokes = collectStrokes(inner, defW, name);
  if (!strokes.length) continue;
  doodles.push({ name, viewBox: viewBoxOf(openTag, strokes[0].pts), strokes });
}

// ---- write + sanity report ----------------------------------------------
mkdirSync(new URL('.', OUT), { recursive: true });
writeFileSync(OUT, JSON.stringify(doodles, null, 2) + '\n', 'utf8');

const strokeCounts = doodles.map((d) => d.strokes.length);
const biggest = doodles[strokeCounts.indexOf(Math.max(...strokeCounts))];
const rel = fileURLToPath(OUT).split(/[\\/]/).slice(-4).join('/');

console.log('--- doodle extractor ---');
console.log(`inline <svg> in body:   ${svgCount}`);
console.log(`doodles emitted:        ${doodles.length}`);
console.log(`total strokes:          ${strokeCounts.reduce((a, b) => a + b, 0)}`);
console.log(`largest stroke count:   ${biggest.name} (${biggest.strokes.length})`);
console.log(`unparsable paths:       ${failures.length}`);
for (const f of failures) console.log('  ! ' + f);
console.log(`wrote:                  ${rel}`);
console.log('names: ' + doodles.map((d) => d.name).join(', '));
