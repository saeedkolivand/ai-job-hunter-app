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
  mkdirSync(join(repoDir, 'docs', 'knowledge', 'decision-records'), { recursive: true });

  writeFileSync(
    join(repoDir, 'package.json'),
    JSON.stringify({ devDependencies: { turbo: '^2.0.0' } })
  );
  writeFileSync(
    join(repoDir, 'apps', 'desktop', 'package.json'),
    JSON.stringify({
      dependencies: { react: '^19.2.8', '@ajh/shared': 'workspace:*' },
    })
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
  writeFileSync(
    join(repoDir, 'docs', 'knowledge', 'decision-records', '0001-real-adr.md'),
    '# real ADR'
  );
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
    '  lastReviewed: string;',
    '}',
    "export const QUADRANTS = [{ id: 'renderer-ui', label: 'Renderer & UI' }];",
    "export const RINGS = [{ id: 'adopt', label: 'Adopt', blurb: 'x' }];",
    'export const RADAR: readonly TechRadarEntry[] = [',
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
        lastReviewed: '2026-08-05',
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
        lastReviewed: '2026-08-05',
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
        lastReviewed: '2026-08-05',
      },
      {
        id: 'mockall',
        name: 'mockall',
        ring: 'adopt',
        quadrant: 'backend-data',
        subjectKind: 'dependency',
        dependencyName: 'mockall',
        lastReviewed: '2026-08-05',
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
        lastReviewed: '2026-08-05',
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
        lastReviewed: '2026-08-05',
      },
      {
        id: 'ollama',
        name: 'Ollama',
        ring: 'adopt',
        quadrant: 'backend-data',
        subjectKind: 'service',
        lastReviewed: '2026-08-05',
      },
      {
        id: 'a-pattern',
        name: 'A Pattern',
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'technique',
        lastReviewed: '2026-08-05',
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
        lastReviewed: '2026-08-05',
      },
      {
        id: 'next-entry',
        name: 'Next Entry',
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'dependency',
        dependencyName: 'zod',
        lastReviewed: '2026-08-05',
      },
    `);
    expect(exitCode).toBe(0);
    expect(output).toContain('2 entries checked');
  });

  it('does not let an apostrophe inside a backtick-quoted field swallow the next entry', () => {
    // The exact failure mode from review: a template literal's apostrophe
    // opens a bogus '-string if backticks aren't tracked as their own quote
    // type, which then runs past this entry's real closing brace and eats
    // the entry that follows.
    const { exitCode, output } = run(`
      {
        id: 'backtick-entry',
        name: 'Backtick Entry',
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'dependency',
        dependencyName: 'react',
        rationale: \`it's fine, and also { has a brace }\`,
        lastReviewed: '2026-08-05',
      },
      {
        id: 'swallowed-entry',
        name: 'Should Not Be Swallowed',
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'dependency',
        dependencyName: 'this-package-does-not-exist',
        lastReviewed: '2026-08-05',
      },
    `);
    expect(exitCode).toBe(1);
    // If the backtick swallowed entry 2, this would report "parsed zero
    // entries" / a count mismatch instead of entry 2's OWN dependency
    // failure by name — that's what proves it wasn't swallowed.
    expect(output).not.toMatch(/silently skipped|couldn't locate/i);
    expect(output).toContain(
      '"Should Not Be Swallowed" names dependency "this-package-does-not-exist"'
    );
  });

  it('does not let an apostrophe inside a // comment swallow the next entry', () => {
    const { exitCode, output } = run(`
      {
        id: 'commented-entry',
        name: 'Commented Entry',
        // don't treat this apostrophe as opening a string
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'dependency',
        dependencyName: 'react',
        lastReviewed: '2026-08-05',
      },
      {
        id: 'after-comment',
        name: 'After Comment',
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'dependency',
        dependencyName: 'this-package-does-not-exist',
        lastReviewed: '2026-08-05',
      },
    `);
    expect(exitCode).toBe(1);
    expect(output).not.toMatch(/silently skipped|couldn't locate/i);
    expect(output).toContain('"After Comment" names dependency "this-package-does-not-exist"');
  });

  it('fails loudly (not silently OK) when an entry whose first key is not id gets skipped by the parser', () => {
    // name-before-id means extractObjectBlocks's startRe never matches this
    // entry's opening brace — exactly the partial-drift scenario the raw
    // id-line count exists to catch. Must NOT report a clean "1 entries
    // checked" as if this entry didn't exist.
    const { exitCode, output } = run(`
      {
        name: 'Id Is Not First',
        id: 'id-not-first',
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'technique',
        lastReviewed: '2026-08-05',
      },
    `);
    expect(exitCode).toBe(1);
    expect(output).toMatch(/silently skipped/i);
    expect(output).toContain("has 1 'id:' line(s) but the parser only extracted 0 entrie(s)");
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
        lastReviewed: '2026-08-05',
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
        lastReviewed: '2026-08-05',
      },
    `);
    expect(broken.exitCode).toBe(1);
    expect(broken.output).toContain('9999-does-not-exist');
  });

  it('fails on a duplicate id', () => {
    const { exitCode, output } = run(`
      {
        id: 'dupe',
        name: 'First',
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'technique',
        lastReviewed: '2026-08-05',
      },
      {
        id: 'dupe',
        name: 'Second',
        ring: 'adopt',
        quadrant: 'backend-data',
        subjectKind: 'technique',
        lastReviewed: '2026-08-05',
      },
    `);
    expect(exitCode).toBe(1);
    expect(output).toContain('duplicate id "dupe"');
    expect(output).toContain('First');
    expect(output).toContain('Second');
  });

  it('fails on a malformed lastReviewed and on a missing one', () => {
    const malformed = run(`
      {
        id: 'a',
        name: 'A',
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'technique',
        lastReviewed: '08/05/2026',
      },
    `);
    expect(malformed.exitCode).toBe(1);
    expect(malformed.output).toContain('lastReviewed');

    const missing = run(`
      {
        id: 'a',
        name: 'A',
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'technique',
      },
    `);
    expect(missing.exitCode).toBe(1);
    expect(missing.output).toContain('lastReviewed');
  });

  it('passes when a claimed major version in name matches the manifest', () => {
    // fixture package.json declares react at ^19.2.8 (major 19)
    const { exitCode } = run(`
      {
        id: 'react',
        name: 'React 19',
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'dependency',
        dependencyName: 'react',
        lastReviewed: '2026-08-05',
      },
    `);
    expect(exitCode).toBe(0);
  });

  it('fails when a claimed major version in name does not match the manifest', () => {
    const { exitCode, output } = run(`
      {
        id: 'react',
        name: 'React 20',
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'dependency',
        dependencyName: 'react',
        lastReviewed: '2026-08-05',
      },
    `);
    expect(exitCode).toBe(1);
    expect(output).toContain('React 20');
    expect(output).toContain('claims major version 20');
    expect(output).toContain('major 19');
  });

  it('does not check a version for a name with no trailing major (finer detail belongs in rationale)', () => {
    const { exitCode } = run(`
      {
        id: 'react',
        name: 'React (concurrent rendering)',
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'dependency',
        dependencyName: 'react',
        rationale: 'Pinned to 19.2.8 today — see lastReviewed for when that was last true.',
        lastReviewed: '2026-08-05',
      },
    `);
    expect(exitCode).toBe(0);
  });

  it('does not fail on an unparseable manifest range (workspace:*) — falls back to skipping the version check', () => {
    // @ajh/shared is declared as "workspace:*" in the fixture repo — the
    // package DOES exist (so the existence check passes), but its range has
    // no leading digit, so parseMajor must return null and the version
    // comparison must be skipped rather than crash or false-fail.
    const { exitCode } = run(`
      {
        id: 'internal',
        name: 'Internal Package 3',
        ring: 'adopt',
        quadrant: 'renderer-ui',
        subjectKind: 'dependency',
        dependencyName: '@ajh/shared',
        lastReviewed: '2026-08-05',
      },
    `);
    expect(exitCode).toBe(0);
  });
});
