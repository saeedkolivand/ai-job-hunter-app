// Generates the Product Hunt gallery images (1270x760 @2x) from a single template.
// Run: node branding/marketing/build.mjs
// Output: 01-hero.png ... 04-private-ai.png in this folder. First image = social preview.
// Renders via headless Chrome (no extra deps). Edit SLIDES below to tweak copy.

import { readFileSync, writeFileSync, mkdtempSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const W = 1270,
  H = 760,
  SCALE = 2;

const CHROME = [
  'C:/Program Files (x86)/Google/Chrome/Application/chrome.exe',
  'C:/Program Files/Google/Chrome/Application/chrome.exe',
  'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
].find((p) => {
  try {
    readFileSync(p);
    return true;
  } catch {
    return false;
  }
});
if (!CHROME) throw new Error('No Chrome/Edge found');

const icon = readFileSync(join(HERE, 'icon-512.png')).toString('base64');

const SLIDES = [
  {
    file: '01-hero',
    eyebrow: 'LOCAL-FIRST · AI-NATIVE',
    title: ['Your AI job-hunting', 'copilot — <k>on your machine</k>.'],
    sub: 'Find jobs, match them to your résumé, and generate tailored applications. Your data and credentials never leave your device.',
    pills: ['macOS · Windows · Linux', 'Works fully offline', '16 job boards'],
    hero: true,
  },
  {
    file: '02-find-match',
    eyebrow: 'FIND & MATCH',
    title: ['Scrapes 16 job boards.', 'Scores every posting <k>against your résumé</k>.'],
    sub: 'Hybrid semantic + keyword matching, ATS scoring, skill-gap detection, and recommendations — including walled boards (Indeed, Glassdoor, Xing) via the aggregator, plus Greenhouse & Lever.',
    pills: ['Semantic match', 'ATS score', 'Skill-gap analysis', 'Autopilot workflows'],
  },
  {
    file: '03-generate',
    eyebrow: 'GENERATE',
    title: ['Tailored résumés, cover letters', '& answers — <k>in seconds</k>.'],
    sub: '9 ATS-safe templates. Export to DOCX, PDF or TXT. Watch the model reason live across every provider. Generate in 11 languages.',
    pills: ['9 ATS-safe templates', 'Live thinking view', 'DOCX / PDF / TXT', '11 languages'],
  },
  {
    file: '04-private-ai',
    eyebrow: 'PRIVATE BY DESIGN',
    title: ['Bring your own AI —', 'or run <k>100% offline</k>.'],
    sub: 'Ollama, OpenAI, Anthropic, Gemini, or local CLI agents (Claude Code, Codex, Gemini CLI). Credentials in your OS keychain. Local SQLite database. Zero telemetry.',
    pills: ['Ollama (offline)', 'OpenAI · Anthropic · Gemini', 'CLI agents', 'Zero telemetry'],
  },
];

const page = (s, n) => `<!doctype html><meta charset=utf-8>
<style>
  *{margin:0;padding:0;box-sizing:border-box}
  html,body{width:${W}px;height:${H}px;overflow:hidden}
  body{
    font-family:"Segoe UI Variable Display","Segoe UI",system-ui,-apple-system,sans-serif;
    color:#eaf5f4; position:relative;
    background:
      radial-gradient(120% 90% at 88% -10%, rgba(91,184,183,.22), transparent 55%),
      radial-gradient(90% 80% at -5% 110%, rgba(43,122,120,.16), transparent 60%),
      linear-gradient(160deg,#0c1716 0%,#081110 55%,#060c0b 100%);
  }
  /* faint dot grid texture */
  body::before{content:"";position:absolute;inset:0;
    background-image:radial-gradient(rgba(137,204,203,.07) 1px,transparent 1px);
    background-size:34px 34px; mask:radial-gradient(120% 100% at 80% 0%,#000,transparent 75%);}
  .frame{position:absolute;inset:0;padding:84px 96px;display:flex;flex-direction:column}
  .top{display:flex;align-items:center;gap:16px}
  .top img{width:52px;height:52px;border-radius:13px;
    box-shadow:0 6px 22px rgba(0,0,0,.5),0 0 0 1px rgba(137,204,203,.18)}
  .brand{font-size:21px;font-weight:700;letter-spacing:-.01em}
  .brand span{color:#5bb8b7}
  .eyebrow{margin-top:auto;font-family:"Cascadia Code",Consolas,ui-monospace,monospace;
    font-size:16px;letter-spacing:.34em;color:#7fd0cf;text-transform:uppercase}
  h1{font-size:${s.hero ? 70 : 60}px;line-height:1.04;letter-spacing:-.025em;font-weight:800;
    margin:18px 0 0;max-width:18ch;text-wrap:balance}
  h1 k{color:#5bb8b7;font-style:normal;
    background:linear-gradient(180deg,transparent 62%,rgba(91,184,183,.20) 62%)}
  .sub{margin-top:24px;font-size:24px;line-height:1.5;color:#aec7c5;max-width:46ch;font-weight:400}
  .pills{display:flex;flex-wrap:wrap;gap:13px;margin-top:36px}
  .pill{font-size:18px;font-weight:600;color:#d6ece9;padding:12px 20px;border-radius:999px;
    background:rgba(91,184,183,.08);border:1px solid rgba(137,204,203,.26);
    backdrop-filter:blur(4px)}
  .foot{position:absolute;left:96px;right:96px;bottom:46px;display:flex;justify-content:space-between;
    align-items:center;font-family:"Cascadia Code",Consolas,ui-monospace,monospace;
    font-size:16px;color:#5e7e7c;letter-spacing:.02em}
  .foot b{color:#89cccb;font-weight:600}
  /* big ghost number, decorative */
  .ghost{position:absolute;right:64px;top:50%;transform:translateY(-50%);
    font-size:520px;font-weight:800;line-height:1;color:rgba(91,184,183,.05);
    user-select:none;pointer-events:none}
  /* concentric rings echoing the icon face, lower-right */
  .rings{position:absolute;right:-160px;bottom:-160px;width:620px;height:620px;
    border-radius:50%;border:1.5px solid rgba(137,204,203,.10);
    box-shadow:0 0 0 70px rgba(137,204,203,.045),0 0 0 150px rgba(137,204,203,.03);
    pointer-events:none}
</style>
<div class="rings"></div>
<div class="ghost">${n}</div>
<div class="frame">
  <div class="top"><img src="data:image/png;base64,${icon}"><div class="brand">AI Job <span>Hunter</span></div></div>
  <div class="eyebrow">${s.eyebrow}</div>
  <h1>${s.title.join('<br>')}</h1>
  <div class="sub">${s.sub}</div>
  <div class="pills">${s.pills.map((p) => `<div class="pill">${p}</div>`).join('')}</div>
</div>
<div class="foot"><span>aijobhunter.app</span><span><b>Free</b> · download for macOS · Windows · Linux</span></div>`;

const profile = mkdtempSync(join(tmpdir(), 'ph-'));
for (let i = 0; i < SLIDES.length; i++) {
  const s = SLIDES[i];
  const html = join(profile, `${s.file}.html`);
  const out = join(HERE, `${s.file}.png`);
  writeFileSync(html, page(s, i + 1));
  execFileSync(
    CHROME,
    [
      '--headless=new',
      '--disable-gpu',
      '--no-sandbox',
      '--hide-scrollbars',
      `--force-device-scale-factor=${SCALE}`,
      `--window-size=${W},${H}`,
      `--user-data-dir=${profile}/c`,
      '--virtual-time-budget=800',
      `--screenshot=${out}`,
      `file:///${html.replace(/\\/g, '/')}`,
    ],
    { stdio: 'ignore' }
  );
  console.log('rendered', `${s.file}.png`);
}
