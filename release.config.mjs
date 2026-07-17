/**
 * release.config.mjs — Semantic Release configuration with contributor credits.
 *
 * ESM module to enable top-level await for resolving the preset transform.
 * The writerOpts.transform WRAPS (not replaces) the preset's transform to preserve
 * type→section bucketing, hidden commit filtering, reference linkification, and
 * BREAKING CHANGE handling while adding contributor credit attribution.
 */

import presetFactory from 'conventional-changelog-conventionalcommits';

import releaseNotesTransform from './scripts/release-notes-transform.cjs';

const { extractGitHubLogin } = releaseNotesTransform;

// Preset config — passed to both the plugin and the factory for consistency
const PRESET_CONFIG = {
  types: [
    { type: 'feat', section: '✨ Features', hidden: false },
    { type: 'fix', section: '🐛 Bug Fixes', hidden: false },
    { type: 'perf', section: '⚡ Performance', hidden: false },
    { type: 'ui', section: '🎨 UI/UX', hidden: false },
    { type: 'refactor', section: '♻️ Refactors', hidden: false },
    { type: 'docs', section: '📚 Documentation', hidden: false },
    { type: 'build', section: '🔧 Build System', hidden: true },
    { type: 'ci', section: '👷 CI', hidden: true },
    { type: 'test', section: '✅ Tests', hidden: true },
    { type: 'chore', section: '🧹 Maintenance', hidden: true },
    { type: 'revert', section: '⏪ Reverts', hidden: false },
  ],
};

/**
 * Resolve the preset and extract its transform function.
 * Called once at startup via top-level await.
 *
 * @returns {Promise<Function>} The preset's original transform(commit, context)
 */
async function resolvePresetTransform() {
  const preset = await presetFactory(PRESET_CONFIG);
  // The preset returns { parser, writer, whatBump } (and historical/version-specific
  // layouts). The writer object (or writerOpts in some preset versions) contains the
  // transform function. Handle both shapes for version robustness.
  const transform = preset?.writer?.transform ?? preset?.writerOpts?.transform;
  if (typeof transform !== 'function') {
    throw new Error(
      'Failed to resolve preset transform from conventional-changelog-conventionalcommits'
    );
  }
  return transform;
}

// Resolve the preset transform at module init time (top-level await)
const presetTransform = await resolvePresetTransform();

/**
 * Wrapper transform: calls the preset's transform first (preserving all its logic:
 * type→section, hidden filtering, reference links, BREAKING CHANGE), then augments
 * the result with contributor credit if applicable.
 *
 * This respects the preset's filtering: if it returns false/null for a hidden commit,
 * we do NOT resurrect it — we return the falsy value as-is.
 *
 * @param {object} commit
 * @param {object} context
 * @returns {object|false|null}
 */
function transform(commit, context) {
  // Capture the GitHub login BEFORE the preset transform may strip/modify the commit
  const login = extractGitHubLogin(commit.authorEmail || commit.author?.email);

  // Call the preset's transform first — it does type→section, filtering, refs, etc.
  const out = presetTransform(commit, context);

  // If the preset filtered it out (hidden commit), respect that decision
  if (!out) {
    return out;
  }

  // Preset kept it. Now augment with contributor credit if applicable.
  if (login && login !== 'saeedkolivand' && !login.includes('[bot]')) {
    out.subject = `${out.subject} (@${login})`;
  }

  return out;
}

export default {
  branches: ['main'],
  tagFormat: 'v${version}',
  preset: 'conventionalcommits',
  initialReleaseVersion: '0.1.0',
  plugins: [
    [
      '@semantic-release/commit-analyzer',
      {
        preset: 'conventionalcommits',
        releaseRules: [
          { type: 'feat', release: 'minor' },
          { type: 'fix', release: 'patch' },
          { type: 'perf', release: 'patch' },
          { type: 'ui', release: 'patch' },
          { type: 'revert', release: 'patch' },
          { type: 'refactor', release: false },
          { type: 'docs', release: false },
          { type: 'style', release: false },
          { type: 'test', release: false },
          { type: 'build', release: false },
          { type: 'ci', release: false },
          { type: 'chore', release: false },
          { breaking: true, release: 'minor' },
        ],
      },
    ],
    [
      '@semantic-release/release-notes-generator',
      {
        preset: 'conventionalcommits',
        presetConfig: PRESET_CONFIG,
        writerOpts: { transform },
      },
    ],
    './scripts/guard-empty-release-notes.cjs',
    [
      '@semantic-release/exec',
      {
        prepareCmd: 'node scripts/sync-tauri-version.cjs ${nextRelease.version}',
      },
    ],
    [
      '@semantic-release/changelog',
      {
        changelogFile: 'CHANGELOG.md',
        changelogTitle: '# Changelog',
      },
    ],
    [
      '@semantic-release/github',
      {
        successComment: '🎉 This is included in version ${nextRelease.version}.',
        failComment: false,
        addReleases: 'bottom',
      },
    ],
    [
      '@semantic-release/git',
      {
        assets: [
          'CHANGELOG.md',
          'package.json',
          'apps/desktop/package.json',
          'apps/extension/package.json',
          'apps/desktop/src-tauri/Cargo.toml',
          'apps/desktop/src-tauri/Cargo.lock',
          'apps/desktop/src-tauri/tauri.conf.json',
          'README.md',
        ],
        message: 'chore(release): ${nextRelease.version} [skip ci]',
      },
    ],
  ],
};
