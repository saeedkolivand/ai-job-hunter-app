import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const scriptPath = join(__dirname, 'check-tech-radar.mjs');

// A minimal fake repo tree: enough package.json/Cargo.toml/tech-radar.ts/ADR
// structure for the script's own hardcoded relative paths to resolve. Every
// test writes its own tech-radar.ts into this tree and runs the real script
// against it via execFileSync (cwd: repoDir) — this is a black-box test of
// the CLI, not of an internal export, mirroring
// scripts/ci-review-verdict.test.mjs / scripts/bump-last-updated.test.mjs.
const repoDir = join(tmpdir(), `check-tech-radar-test-${Date.now()}`);

function writeBaseRepo() {
  mkdirSync(join(repoDir, 'apps', 'landing', 'src', 'data'), { recursive: true });
  mkdirSync(join(repoDir, 'apps', 'desktop', 'src-tauri'), { recursive: true });
  mkdirSync(join(repoDir, 'packages', 'shared'), { recursive: true });
  mkdirSync(join(repoDir, 'docs', 'adr'), { recursive: true });

  writeFileSync(
    join(repoDir, 'package.json'),
    JSON.stringify({ devDependencies: { turbo: '^2.0.0' } })
  );
  writeFileSync(
    join(repoDir, 'apps', 'desktop', 'package.json'),
    JSON.stringify({ dependencies: { react: '^19.0.0' } })
  );
  writeFileSync(
    join(repoDir, 'packages', 'shared', 'package.json'),
    JSON.stringify({ dependencies: { zod: '^4.0.0' } })
  );
  writeFileSync(
    join(repoDir, 'apps', 'desktop', 'src-tauri', 'Cargo.toml'),
    [
      '[package]',
      'name = "fake"',
      '',
      '[dependencies]',
      '# a comment line with = in it should never be read as a dep',
      'tauri = { version = "2" }',
      'serde = { version = "1", features = ["derive"] }',
      '',
      "[target.'cfg(windows)'.dependencies]",
      'windows = "0.62"',
      '',
      '[dev-dependencies]',
      'mockall = "0.15"',
      '',
      '[[bin]]',
      'name = "not-a-dependency"',
    ].join('\n')
  );
  writeFileSync(join(repoDir, 'docs', 'adr', '0001-real-adr.md'), '# real ADR');
}

/** Write tech-radar.ts with the given RADAR array body and run the checker. */
function run(entriesSource) {
  const source = [
    "export type RadarRing = 'adopt' | 'trial' | 'assess' | 'hold';",
    "export type RadarQuadrant = 'renderer-ui' | 'backend-data' | 'documents-export' | 'build-ship-trust';",
    "export type RadarSubjectKind = 'dependency' | 'technique' | 'service' | 'not-adopted';",
    'export interface TechRadarEntry {',
    '  id: string;',
    '  name: string;',
    '  ring: RadarRing;',
    '  quadrant: RadarQuadrant;',
    '  subjectKind: RadarSubjectKind;',
    '  dependencyName?: string;',
    '  adrSlug?: string;',
    '}',
    "export const QUADRANTS = [{ id: 'renderer-ui', label: 'Renderer & UI' }];",
    "export const RINGS = [{ id: 'adopt', label: 'Adopt', blurb: 'x' }];",
    'export const RADAR = [',
    entriesSource,
    '];',
  ].join('\n');
  writeFileSync(join(repoDir, 'apps', 'landing', 'src', 'data', 'tech-radar.ts'), source);
  try {
    const stdout = execFileSync('node', [scriptPath], { cwd: repoDir, encoding: 'utf8' });
    return { exitCode: 0, output: stdout };
  } catch (err) {
    return { exitCode: err.status, output: `${err.stdout ?? ''}${err.stderr ?? ''}` };
  }
}

describe('check-tech-radar', () => {
  beforeEach(() => writeBaseRepo());
  afterEach(() => rmSync(repoDir, { recursive: true, force: true }));

  it('passes when every dependency entry names a real dependency', () => {
    const { exitCode, output } = run(`
      {
        id: 'tauri',
        name: 'Tauri',
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'dependency',
        dependencyName: 'tauri',
      },
    `);
    expect(exitCode).toBe(0);
    expect(output).toContain('OK');
  });

  it("finds a Cargo.toml dependency under a [target.'cfg(...)'.dependencies] table", () => {
    const { exitCode } = run(`
      {
        id: 'windows-crate',
        name: 'windows',
        ring: 'adopt',
        quadrant: 'backend-data',
        subjectKind: 'dependency',
        dependencyName: 'windows',
      },
    `);
    expect(exitCode).toBe(0);
  });

  it('finds a package.json dependency from any workspace, and a Cargo dev-dependency', () => {
    const { exitCode } = run(`
      {
        id: 'zod',
        name: 'Zod',
        ring: 'adopt',
        quadrant: 'backend-data',
        subjectKind: 'dependency',
        dependencyName: 'zod',
      },
      {
        id: 'mockall',
        name: 'mockall',
        ring: 'adopt',
        quadrant: 'backend-data',
        subjectKind: 'dependency',
        dependencyName: 'mockall',
      },
    `);
    expect(exitCode).toBe(0);
  });

  it('fails when a dependency entry names a package that does not exist anywhere', () => {
    const { exitCode, output } = run(`
      {
        id: 'ghost',
        name: 'Ghost Package',
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'dependency',
        dependencyName: 'this-package-does-not-exist',
      },
    `);
    expect(exitCode).toBe(1);
    expect(output).toContain('Ghost Package');
    expect(output).toContain('this-package-does-not-exist');
    expect(output).toMatch(/update|remove|delete/i);
  });

  it('never checks a technique/service/not-adopted entry against manifests', () => {
    const { exitCode } = run(`
      {
        id: 'xstate',
        name: 'XState',
        ring: 'hold',
        quadrant: 'renderer-ui',
        subjectKind: 'not-adopted',
      },
      {
        id: 'ollama',
        name: 'Ollama',
        ring: 'adopt',
        quadrant: 'backend-data',
        subjectKind: 'service',
      },
      {
        id: 'a-pattern',
        name: 'A Pattern',
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'technique',
      },
    `);
    expect(exitCode).toBe(0);
  });

  it('does not let literal braces inside a rationale string break entry parsing', () => {
    const { exitCode, output } = run(`
      {
        id: 'motion',
        name: 'Motion',
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'dependency',
        dependencyName: 'react',
        rationale: 'ESLint blocks an inline { duration, ease } object anywhere in feature code.',
      },
      {
        id: 'next-entry',
        name: 'Next Entry',
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'dependency',
        dependencyName: 'zod',
      },
    `);
    expect(exitCode).toBe(0);
    expect(output).toContain('2 entries checked');
  });

  it('passes on a real adrSlug and fails on a broken one', () => {
    const ok = run(`
      {
        id: 'a',
        name: 'A',
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'technique',
        adrSlug: '0001-real-adr',
      },
    `);
    expect(ok.exitCode).toBe(0);

    const broken = run(`
      {
        id: 'a',
        name: 'A',
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'technique',
        adrSlug: '9999-does-not-exist',
      },
    `);
    expect(broken.exitCode).toBe(1);
    expect(broken.output).toContain('9999-does-not-exist');
  });
});
