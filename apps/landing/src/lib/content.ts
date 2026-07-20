import { readFileSync } from 'node:fs';
import { join } from 'node:path';

// Server-only: read a verbatim page fragment (extracted from the original
// hand-authored HTML) at build time. `output: export` renders every page at
// build in Node, so `fs` is available and the bytes are inlined into the static
// HTML — a faithful, drift-free port of the gag-heavy markup without hand-
// converting `{`/entity-laden copy into JSX. cwd is the app dir during
// `next build` / `next dev`.
const CONTENT_ROOT = join(process.cwd(), 'src', 'content');

export function readContent(route: string, file: string): string {
  return readFileSync(join(CONTENT_ROOT, route, file), 'utf8');
}
