import { existsSync, mkdirSync } from 'node:fs';
import { chromium } from 'playwright';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, '..');
const cardPath = join(root, 'landing', 'social-card.html');
if (!existsSync(cardPath)) {
  console.error('missing', cardPath);
  process.exit(1);
}
const card = pathToFileURL(cardPath).href;

const targets = [
  { out: 'landing/og-card.jpg', w: 1200, h: 630 },
  { out: 'branding/github-social-preview.png', w: 1280, h: 640 },
];

const browser = await chromium.launch();
try {
  for (const t of targets) {
    const page = await browser.newPage({
      viewport: { width: t.w, height: t.h },
      deviceScaleFactor: 1,
    });
    await page.goto(card, { waitUntil: 'networkidle' });
    await page.evaluate(() => document.fonts.ready);
    const outPath = join(root, t.out);
    mkdirSync(dirname(outPath), { recursive: true });
    const isJpeg = /\.jpe?g$/i.test(outPath);
    await page.screenshot(
      isJpeg ? { path: outPath, type: 'jpeg', quality: 82 } : { path: outPath }
    );
    await page.close();
    console.log('wrote', t.out, t.w + 'x' + t.h);
  }
} finally {
  await browser.close();
}
