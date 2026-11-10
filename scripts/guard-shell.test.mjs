import { expect, test } from 'vitest';

import { gate } from '../.claude/hooks/critic-report-gate.mjs';
import { isCriticAgent, verdict } from '../.claude/hooks/guard-shell.mjs';

test('nobody stops the owner app, main session included', () => {
  for (const cmd of [
    'taskkill /IM ajh-tauri.exe /F',
    'Stop-Process -Name ajh-tauri -Force',
    'Get-Process ajh-tauri | Stop-Process',
    'pkill -f ajh-tauri',
    // macOS / Linux installed builds
    'killall "AI Job Hunter"',
    'pkill -f "AI Job Hunter.app"',
    `osascript -e 'quit app "AI Job Hunter"'`,
    'kill $(pgrep -f ai-job-hunter)',
    'pgrep ajh-tauri | xargs kill -9',
  ])
    expect(verdict(cmd, undefined, false), cmd).toBeTruthy();
  for (const cmd of [
    'tasklist | rg ajh-tauri',
    'ps aux | grep "AI Job Hunter"',
    'pgrep -l ai-job-hunter',
  ])
    expect(verdict(cmd, undefined, false), cmd).toBeNull();
});

test('subagents never change git state; reads stay allowed', () => {
  for (const cmd of [
    'git commit -m x',
    'git -c core.hooksPath=/dev/null commit -q',
    'git stash push',
    'git checkout -- a.rs',
    'git push',
  ])
    expect(verdict(cmd, 'rust-backend-author', false), cmd).toBeTruthy();
  for (const cmd of [
    'git diff --stat',
    'git log --oneline -5',
    'git show HEAD:a.rs',
    'git worktree list',
  ])
    expect(verdict(cmd, 'rust-backend-author', false), cmd).toBeNull();
  expect(verdict('git commit -m x', undefined, false)).toBeNull();
});

test('critics are read-only in the shell; authors are not', () => {
  for (const cmd of [
    "sed -i 's/a/b/' x.rs",
    'rm -rf target/x',
    'cd a && tee b.txt',
    'Set-Content x.ts y',
  ])
    expect(verdict(cmd, 'pr-reviewer', true), cmd).toBeTruthy();
  for (const cmd of [
    'rg "fn x() -> u8" src',
    'cargo test --lib sync 2>&1 | tail -3',
    "sed -n '1,9p' a.rs",
  ])
    expect(verdict(cmd, 'pr-reviewer', true), cmd).toBeNull();
  expect(verdict("sed -i 's/a/b/' x.rs", 'rust-backend-author', false)).toBeNull();
});

test('critic detection reads the repo agent tools line', () => {
  expect(isCriticAgent('pr-reviewer')).toBe(true);
  expect(isCriticAgent('rust-backend-author')).toBe(false);
  expect(isCriticAgent('general-purpose')).toBe(false);
});

test('a critic APPROVE without a self-red-team goes back to the critic', () => {
  expect(gate('VERDICT: APPROVE. No findings.', 'pr-reviewer', true)).toBeTruthy();
  expect(
    gate('VERDICT: APPROVE\n## Self-red-team\nattacked and held: x', 'pr-reviewer', true)
  ).toBeNull();
  expect(gate('VERDICT: REQUEST CHANGES — HIGH at a.rs:3', 'pr-reviewer', true)).toBeNull();
  expect(gate('APPROVE', 'rust-backend-author', false)).toBeNull();
  expect(gate('APPROVE', undefined, false)).toBeNull();
});
