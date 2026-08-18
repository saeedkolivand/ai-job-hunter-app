import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const scriptPath = join(__dirname, 'check-toolchain-pin.mjs');

// Black-box test of the CLI against a minimal fake repo tree, mirroring
// scripts/check-tech-radar.test.mjs. Every case writes its own
// rust-toolchain.toml / workflow / Cargo.toml and runs the real script with
// cwd: repoDir, so the script's own hardcoded relative paths resolve.
const repoDir = join(tmpdir(), `check-toolchain-pin-test-${Date.now()}`);

function writeRepo({ channel = '1.97.1', uses = [], rustVersion = '1.95' } = {}) {
  mkdirSync(join(repoDir, 'apps', 'desktop', 'src-tauri'), { recursive: true });
  mkdirSync(join(repoDir, '.github', 'workflows'), { recursive: true });
  mkdirSync(join(repoDir, '.github', 'actions', 'setup-rust'), { recursive: true });

  writeFileSync(
    join(repoDir, 'apps', 'desktop', 'src-tauri', 'rust-toolchain.toml'),
    channel === null ? '# no channel here\n' : `[toolchain]\nchannel = "${channel}"\n`
  );
  writeFileSync(
    join(repoDir, 'apps', 'desktop', 'src-tauri', 'Cargo.toml'),
    ['[package]', 'name = "fake"', `rust-version = "${rustVersion}"`, ''].join('\n')
  );

  const steps = uses
    .map((u) => `      - uses: dtolnay/rust-toolchain@${u.rev ?? 'deadbeef'}${u.comment ?? ''}`)
    .join('\n');
  writeFileSync(
    join(repoDir, '.github', 'workflows', 'ci.yml'),
    ['jobs:', '  build:', '    steps:', steps, ''].join('\n')
  );
}

/** Run the checker; returns {code, out}. */
function run() {
  try {
    const out = execFileSync('node', [scriptPath], { cwd: repoDir, encoding: 'utf8' });
    return { code: 0, out };
  } catch (e) {
    return { code: e.status ?? 1, out: `${e.stdout ?? ''}${e.stderr ?? ''}` };
  }
}

beforeEach(() => rmSync(repoDir, { recursive: true, force: true }));
afterEach(() => rmSync(repoDir, { recursive: true, force: true }));

describe('check-toolchain-pin', () => {
  it('passes when the toml channel and every action comment agree', () => {
    writeRepo({ uses: [{ comment: ' # 1.97.1' }, { comment: ' # 1.97.1' }] });
    const { code, out } = run();
    expect(code, out).toBe(0);
    expect(out).toContain('1.97.1');
  });

  // The mistake the guard exists for: two places, one bumped.
  it('fails when the toml is bumped but the action pin is not', () => {
    writeRepo({ channel: '1.98.0', uses: [{ comment: ' # 1.97.1' }] });
    const { code, out } = run();
    expect(code).toBe(1);
    expect(out).toMatch(/pinned to 1\.97\.1, but .*rust-toolchain\.toml pins 1\.98\.0/);
  });

  it('fails when the action pin is bumped but the toml is not', () => {
    writeRepo({ channel: '1.97.1', uses: [{ comment: ' # 1.98.0' }] });
    expect(run().code).toBe(1);
  });

  // One agreeing use must not excuse a second disagreeing one.
  it('checks EVERY use, not just the first', () => {
    writeRepo({ uses: [{ comment: ' # 1.97.1' }, { comment: ' # 1.96.0' }] });
    const { code, out } = run();
    expect(code).toBe(1);
    expect(out).toContain('1.96.0');
  });

  it('rejects a floating channel, which is what the pin exists to prevent', () => {
    for (const channel of ['stable', 'beta', 'nightly', '1.97']) {
      writeRepo({ channel, uses: [{ comment: ' # 1.97.1' }] });
      const { code, out } = run();
      expect(code, `channel ${channel} should be rejected`).toBe(1);
      expect(out).toContain('not an exact version');
    }
  });

  it('rejects an opaque SHA with no version comment', () => {
    writeRepo({ uses: [{ rev: 'abc123', comment: '' }] });
    const { code, out } = run();
    expect(code).toBe(1);
    expect(out).toContain('no trailing');
  });

  it('accepts a v-prefixed version comment', () => {
    writeRepo({ uses: [{ comment: ' # v1.97.1' }] });
    expect(run().code).toBe(0);
  });

  // Removing CI's pin entirely is a bigger regression than mismatching it,
  // and would otherwise pass vacuously — nothing to disagree with.
  it('fails when no use is found at all rather than passing vacuously', () => {
    writeRepo({ uses: [] });
    const { code, out } = run();
    expect(code).toBe(1);
    expect(out).toContain('no dtolnay/rust-toolchain use found');
  });

  it('fails when the MSRV is newer than the pinned toolchain', () => {
    writeRepo({ channel: '1.97.1', rustVersion: '1.99', uses: [{ comment: ' # 1.97.1' }] });
    const { code, out } = run();
    expect(code).toBe(1);
    expect(out).toContain('cannot compile');
  });

  it('allows an MSRV below the pin — the normal, correct case', () => {
    writeRepo({ channel: '1.97.1', rustVersion: '1.95', uses: [{ comment: ' # 1.97.1' }] });
    expect(run().code).toBe(0);
  });

  // A `uses:` inside a comment is prose about the pin, not a pin.
  it('ignores a commented-out uses line', () => {
    writeRepo({ uses: [{ comment: ' # 1.97.1' }] });
    writeFileSync(
      join(repoDir, '.github', 'workflows', 'note.yml'),
      '# uses: dtolnay/rust-toolchain@old # 1.50.0\n'
    );
    expect(run().code).toBe(0);
  });
});
