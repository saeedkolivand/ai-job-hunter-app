import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// Server-only: read a docs-tier CSS file (design tokens + shell chrome) at build
// time. Resolved relative to THIS module (not process.cwd()) so it works both
// under `next build` (cwd = app dir) and under vitest run from the repo root.
// The bytes are inlined into the exported HTML via <PageStyle>, exactly like the
// per-route content CSS — no global stylesheet, so the marketing tier is untouched.
const STYLES_ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', 'styles');

export function readStyle(file: string): string {
  return readFileSync(join(STYLES_ROOT, file), 'utf8');
}
