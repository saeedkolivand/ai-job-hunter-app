/**
 * The AMO source-archive metadata that a human reviewer actually reads.
 *
 * A Firefox source submission is rejected — or the add-on is pulled — when the
 * archive's README does not name the build environment and the exact commands
 * that reproduce the uploaded package. That README is generated (never typed
 * in) so its tool versions cannot drift from the toolchain that built the
 * artifact; this pins the parts a reviewer depends on, and the archive name the
 * release workflow attaches.
 *
 * The archive's completeness is proved elsewhere and better: the
 * `publish-firefox` job unpacks the real archive, runs these very commands and
 * byte-compares the result against the shipped zip before submitting.
 */
import { describe, expect, it } from 'vitest';

import {
  buildReadme,
  detectOs,
  parseArgs,
  README_ENTRY_NAME,
  sourceArchiveName,
} from '../scripts/source-archive.mjs';
// Deliberately imported from the BUILD CONFIG rather than from the shared list
// the README is generated from: the property under test is that the README
// describes what the build actually emits.
import { INJECTED_ENTRIES } from '../vite.config.mts';

const readme = (over = {}) =>
  buildReadme({
    version: '1.2.3',
    os: 'Ubuntu 24.04.2 LTS',
    node: '22.21.0',
    packageManager: 'pnpm@11.10.0',
    ...over,
  });

describe('sourceArchiveName', () => {
  it('names the archive after the add-on version', () => {
    expect(sourceArchiveName('1.2.3')).toBe('ai-job-hunter-extension-source-1.2.3.zip');
  });

  it('keeps the store zips and the source zip distinguishable by name', () => {
    // `package.mjs` emits ai-job-hunter-extension-{chrome,firefox}-<v>.zip into
    // the same dist/ dir, and the release job globs them separately.
    expect(sourceArchiveName('1.2.3')).not.toMatch(/-(chrome|firefox)-/);
  });
});

describe('buildReadme', () => {
  it('states the build environment it was generated from', () => {
    const text = readme();
    expect(text).toContain('v1.2.3');
    expect(text).toContain('Ubuntu 24.04.2 LTS');
    expect(text).toContain('22.21.0');
  });

  it('gives the pnpm version bare in the table and qualified in the install command', () => {
    const text = readme({ packageManager: 'pnpm@9.0.1' });
    expect(text).toContain('| pnpm | 9.0.1 |');
    expect(text).toContain('corepack prepare pnpm@9.0.1 --activate');
  });

  it('spells out every command needed to reproduce the submitted package', () => {
    const text = readme();
    expect(text).toContain('pnpm install --frozen-lockfile');
    expect(text).toContain('pnpm -F @ajh/shared build');
    expect(text).toContain('pnpm -F @ajh/extension build:firefox');
    expect(text).toContain('apps/extension/dist/firefox');
  });

  it('links out to the tools a reviewer has to install', () => {
    const text = readme();
    expect(text).toContain('https://nodejs.org/en/download');
    expect(text).toContain('https://pnpm.io/installation');
  });

  // A Mozilla reviewer reads this note to understand why some files in an
  // otherwise-minified bundle are readable. The hand-written version of the list
  // silently omitted submit-watch.js, so an entry the build emits had no
  // explanation at all. `readme()` passes no list — this asserts the DEFAULT the
  // script ships with still covers everything vite.config.mts builds.
  it('accounts for every injected script the build emits unminified', () => {
    const text = readme();
    for (const entry of INJECTED_ENTRIES) expect(text).toContain(`\`${entry}.js\``);
    expect(text).toContain(`These ${INJECTED_ENTRIES.length} files`);
  });
});

describe('detectOs', () => {
  it('prefers the distro name when /etc/os-release is readable', () => {
    const osRelease = 'NAME="Ubuntu"\nPRETTY_NAME="Ubuntu 24.04.2 LTS"\nVERSION_ID="24.04"\n';
    expect(detectOs(() => osRelease)).toBe('Ubuntu 24.04.2 LTS');
  });

  it('falls back to the kernel label off Linux instead of throwing', () => {
    expect(
      detectOs(() => {
        throw new Error('ENOENT');
      })
    ).toMatch(/\S+ \S+/);
  });
});

describe('parseArgs', () => {
  it('archives HEAD into the default dist path when given nothing', () => {
    expect(parseArgs([])).toEqual({ ref: 'HEAD', out: undefined });
  });

  it('takes an explicit release tag and output path', () => {
    expect(parseArgs(['--ref', 'v1.2.3', '--out', 'a/b.zip'])).toEqual({
      ref: 'v1.2.3',
      out: 'a/b.zip',
    });
  });
});

describe('README_ENTRY_NAME', () => {
  it('lands at the archive root, where AMO looks for build instructions', () => {
    expect(README_ENTRY_NAME).not.toContain('/');
    expect(README_ENTRY_NAME.endsWith('.md')).toBe(true);
  });
});
