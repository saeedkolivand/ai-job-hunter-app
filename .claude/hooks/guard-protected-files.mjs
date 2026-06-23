#!/usr/bin/env node
// PreToolUse(Edit|Write): block hand-edits to the lockfile. Dependency changes
// must go through `pnpm add/remove` so the resolved graph never silently drifts.
// Exit 2 rejects the tool call; stderr is shown to Claude. Add patterns as needed.
const PROTECTED = [/(^|[\\/])pnpm-lock\.yaml$/];

const raw = await new Promise((resolve) => {
  let data = '';
  process.stdin.on('data', (c) => (data += c));
  process.stdin.on('end', () => resolve(data));
});

const file = JSON.parse(raw || '{}')?.tool_input?.file_path || '';
if (PROTECTED.some((re) => re.test(file))) {
  console.error(
    `Blocked: "${file}" is protected. Change dependencies with "pnpm add/remove", not by editing the lockfile.`
  );
  process.exit(2);
}
