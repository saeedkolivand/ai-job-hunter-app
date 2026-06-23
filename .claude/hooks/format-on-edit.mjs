#!/usr/bin/env node
// PostToolUse(Edit|Write): prettier-format the file Claude just touched.
// Mirrors the lint-staged prettier pass so edits land formatted instead of
// waiting for commit. --ignore-unknown skips non-prettier files; .prettierignore
// is honored automatically. Best-effort: a prettier hiccup never blocks the edit.
import { execSync } from 'node:child_process';

const raw = await new Promise((resolve) => {
  let data = '';
  process.stdin.on('data', (c) => (data += c));
  process.stdin.on('end', () => resolve(data));
});

const file = JSON.parse(raw || '{}')?.tool_input?.file_path;
if (file) {
  // prettier globs its CLI args; on Windows the backslashes are read as glob escapes
  // ("No files matching the pattern"). Forward slashes work and Windows accepts them.
  const f = file.replace(/\\/g, '/');
  try {
    execSync(`pnpm exec prettier --ignore-unknown --write "${f}"`, { stdio: 'ignore' });
  } catch {
    // ponytail: best-effort format; swallow errors so a slow/failed prettier never breaks the session
  }
}
