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
import vitePkg from 'vite/package.json';
import { describe, expect, it } from 'vitest';

import {
  buildReadme,
  detectOs,
  parseArgs,
  README_ENTRY_NAME,
  refMismatch,
  sourceArchiveName,
  viteNodeRange,
} from '../scripts/source-archive.mjs';

/**
 * Hand-written ON PURPOSE — do not replace this with an import.
 *
 * The README's list and `injected-entries.mjs` are the same const, so anchoring
 * the assertion on that const would only prove `x === x`: deleting an entry
 * would shrink both sides and stay green. This literal is the independent side.
 * Adding an entry to `injected-entries.mjs` breaks the count assertion below,
 * and removing one breaks the per-name assertions — so the list a Mozilla
 * reviewer reads cannot silently gain or lose a file. (`build-output.test.ts`
 * separately requires a new entry to be classified as completion-value or
 * global-installing, so a real addition fails in two places, not none.)
 */
const EXPECTED_INJECTED_SCRIPTS = [
  'content.js',
  'fill.js',
  'capture.js',
  'capture-questions.js',
  'capture-rows.js',
  'answer-fill.js',
  'answer-replace.js',
  'submit-watch.js',
  'probe-fields.js',
];

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

  // "any 22.x release works" was simply false — the locked Vite wants
  // `^20.19.0 || >=22.12.0`, so a reviewer on 22.0 would hit a build failure we
  // had told them to expect success from. The range is read from Vite's own
  // `engines`, never written down.
  it('quotes the Node range Vite actually requires', () => {
    expect(readme({ nodeRange: '^20.19.0 || >=22.12.0' })).toContain(
      'Vite pins the supported range at `^20.19.0 || >=22.12.0`'
    );
  });

  it('points at Vite instead of inventing a range when it cannot be read', () => {
    const text = readme({ nodeRange: null });
    expect(text).toContain('`engines.node`');
    expect(text).not.toMatch(/any \d+\.x/);
  });

  // A Mozilla reviewer reads this note to understand why some files in an
  // otherwise-minified bundle are readable; the version that was written by hand
  // omitted submit-watch.js, so a file the build emits had no explanation at all.
  // `readme()` passes no list, so this exercises the DEFAULT the script ships
  // with, against the independent literal above.
  it('names exactly the injected scripts that ship unminified', () => {
    const text = readme();
    for (const file of EXPECTED_INJECTED_SCRIPTS) expect(text).toContain(`\`${file}\``);
    expect(text).toContain(`These ${EXPECTED_INJECTED_SCRIPTS.length} files`);
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

describe('viteNodeRange', () => {
  it('reads the range straight out of the installed Vite', () => {
    expect(viteNodeRange()).toBe(vitePkg.engines.node);
  });

  it('returns null rather than a guess when Vite cannot be resolved', () => {
    expect(
      viteNodeRange(() => {
        throw new Error('MODULE_NOT_FOUND');
      })
    ).toBeNull();
    expect(viteNodeRange(() => ({}))).toBeNull();
    expect(viteNodeRange(() => ({ engines: { node: '  ' } }))).toBeNull();
  });
});

describe('refMismatch', () => {
  const at = (map: Record<string, string>) => (ref: string) => map[ref] ?? null;

  it('allows the default HEAD without asking git anything', () => {
    expect(
      refMismatch('HEAD', () => {
        throw new Error('git must not be called');
      })
    ).toBeNull();
  });

  it('allows a tag that points at the checked-out commit', () => {
    expect(refMismatch('v1.2.3', at({ HEAD: 'abc123', 'v1.2.3^{commit}': 'abc123' }))).toBeNull();
  });

  // The defect: `git archive` would take the TREE from the tag while the
  // version, injected-script list and build README came from the worktree.
  it('refuses a ref that is not the checked-out commit, naming both', () => {
    const message = refMismatch('v1.2.3', at({ HEAD: 'abc123', 'v1.2.3^{commit}': 'def456' }));
    expect(message).toContain('abc123');
    expect(message).toContain('def456');
    expect(message).toContain('v1.2.3');
  });

  it('refuses a ref git cannot resolve', () => {
    expect(refMismatch('nope', at({ HEAD: 'abc123' }))).toContain('not a commit');
  });
});

describe('README_ENTRY_NAME', () => {
  it('lands at the archive root, where AMO looks for build instructions', () => {
    expect(README_ENTRY_NAME).not.toContain('/');
    expect(README_ENTRY_NAME.endsWith('.md')).toBe(true);
  });
});
