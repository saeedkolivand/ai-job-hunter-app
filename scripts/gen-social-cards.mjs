import { chromium } from 'playwright';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, '..');
const card = 'file://' + join(root, 'landing', 'social-card.html').replace(/\\/g, '/');

const targets = [
  { out: 'landing/og-card.png', w: 1200, h: 630 },
  { out: 'branding/github-social-preview.png', w: 1280, h: 640 },
];

const browser = await chromium.launch();
for (const t of targets) {
  const page = await browser.newPage({
    viewport: { width: t.w, height: t.h },
    deviceScaleFactor: 1,
  });
  await page.goto(card, { waitUntil: 'networkidle' });
  await page.evaluate(() => document.fonts.ready);
  await page.screenshot({ path: join(root, t.out) });
  await page.close();
  console.log('wrote', t.out, t.w + 'x' + t.h);
}
await browser.close();
